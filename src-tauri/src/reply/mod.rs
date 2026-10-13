//! "Réponds dans leur langue": the user types in French, Crosstalk translates
//! to the teammates' language (English or Russian) on the CPU and the window
//! copies it to the clipboard, ready to paste in CS2's chat (Y / U, Ctrl+V).
//! Nothing is typed into the game: the user pastes and sends it themselves.
//!
//! Usual calls come from a hand-written phrasebook (`phrases.rs`, exact and
//! instant); free text goes to Opus-MT fr→en / fr→ru (Apache-2.0), loaded the
//! first time it is needed. CS terms and the current map's callouts are kept.

pub mod phrases;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use serde::Serialize;
use tauri::{
    AppHandle, Emitter, LogicalSize, Manager, PhysicalPosition, WebviewUrl, WebviewWindowBuilder,
};

use crate::translation::glossary::{self, Term};
use crate::translation::models;
use crate::translation::service::Engine;

pub const LABEL: &str = "reply";
pub const OPEN_EVENT: &str = "reply://open";
pub const TARGETS: [&str; 2] = ["en", "ru"];
const MAX_CHARS: usize = 300;
const WIDTH: f64 = 560.0;
const HEIGHT: f64 = 340.0;

/// CS words kept as typed (the map's callouts are added while a map is known).
const KEEP: &[&str] = &[
    "smoke", "flash", "molly", "molotov", "AWP", "eco", "rush", "deagle", "AK", "M4", "nade",
    "lurk",
];

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReplyResult {
    pub text: String,
    pub target: String,
    /// `phrasebook` (exact, hand-written) or `model` (Opus-MT).
    pub via: &'static str,
    pub ms: u32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReplyModel {
    pub target: &'static str,
    pub repo: &'static str,
    pub size_mb: u32,
    pub installed: bool,
    pub loaded: bool,
}

pub type Loader = Box<dyn Fn(&str) -> Result<Box<dyn Engine>, String> + Send + Sync>;

pub struct ReplyService {
    models_dir: PathBuf,
    loader: Loader,
    engines: Mutex<HashMap<String, Box<dyn Engine>>>,
    map_terms: Mutex<Vec<String>>,
}

impl ReplyService {
    pub fn new(models_dir: PathBuf) -> Arc<Self> {
        let dir = models_dir.clone();
        Self::with_loader(
            models_dir,
            Box::new(move |target| {
                let p = models::reply_pair(target).ok_or("langue non gérée")?;
                if let Some(f) = models::missing_file(&dir, p) {
                    return Err(format!(
                        "modèle français → {} absent ({f}) : téléchargez-le",
                        target.to_uppercase()
                    ));
                }
                let m = crate::translation::marian::Marian::load(
                    &models::pair_dir(&dir, p),
                    crate::translation::marian::Mode::Fast,
                )?;
                Ok(Box::new(m) as Box<dyn Engine>)
            }),
        )
    }

    pub fn with_loader(models_dir: PathBuf, loader: Loader) -> Arc<Self> {
        Arc::new(Self {
            models_dir,
            loader,
            engines: Mutex::new(HashMap::new()),
            map_terms: Mutex::new(Vec::new()),
        })
    }

    pub fn models(&self) -> Vec<ReplyModel> {
        let engines = self.engines.lock().unwrap();
        models::REPLY
            .iter()
            .map(|p| ReplyModel {
                target: p.target,
                repo: p.repo,
                size_mb: (p.files.iter().map(|f| f.1).sum::<u64>() / 1_000_000) as u32,
                installed: models::missing_file(&self.models_dir, p).is_none(),
                loaded: engines.contains_key(p.target),
            })
            .collect()
    }

    /// Callouts of the map being played, kept as typed.
    pub fn set_map_terms(&self, callouts: Vec<String>) {
        *self.map_terms.lock().unwrap() = callouts;
    }

    pub fn translate(&self, text: &str, target: &str) -> Result<ReplyResult, String> {
        let started = Instant::now();
        if !TARGETS.contains(&target) {
            return Err(format!("langue non gérée : {target}"));
        }
        let text = text.trim();
        if text.is_empty() {
            return Err("message vide".into());
        }
        if text.chars().count() > MAX_CHARS {
            return Err(format!(
                "message trop long (plus de {MAX_CHARS} caractères)"
            ));
        }
        let done = |text: String, via| ReplyResult {
            text,
            target: target.to_string(),
            via,
            ms: started.elapsed().as_millis() as u32,
        };
        if let Some(t) = phrases::lookup(text, target) {
            return Ok(done(t, "phrasebook"));
        }
        let mut engines = self.engines.lock().unwrap();
        if !engines.contains_key(target) {
            let e = (self.loader)(target)?;
            engines.insert(target.to_string(), e);
        }
        let engine = engines.get_mut(target).expect("loaded");
        let terms: Vec<Term> = KEEP
            .iter()
            .map(|s| s.to_string())
            .chain(self.map_terms.lock().unwrap().iter().cloned())
            .map(|source| Term {
                source,
                target: None,
            })
            .collect();
        let (out, _) = glossary::translate(text, &terms, |t| engine.translate(t))?;
        let out = crate::translation::callouts::clean_output(&out);
        if out.is_empty() {
            return Err("traduction vide".into());
        }
        Ok(done(out, "model"))
    }
}

/// Teammates' language: the most frequent of en / ru among recent lines.
pub fn guess_target(recent_langs: &[String]) -> &'static str {
    let ru = recent_langs.iter().filter(|l| l.as_str() == "ru").count();
    let en = recent_langs.iter().filter(|l| l.as_str() == "en").count();
    if ru > en {
        "ru"
    } else {
        "en"
    }
}

/// The (hidden) reply window, created at start-up.
pub fn create_window(app: &AppHandle) -> tauri::Result<()> {
    let win = WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App("reply.html".into()))
        .title("Crosstalk — répondre")
        .inner_size(WIDTH, HEIGHT)
        .transparent(true)
        .decorations(false)
        .shadow(false)
        .resizable(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .focused(false)
        .visible(false)
        .build()?;
    // Bottom centre of the main screen, above CS2's chat line.
    if let Some(m) = win.primary_monitor()? {
        let area = m.work_area();
        let size = LogicalSize::new(WIDTH, HEIGHT).to_physical::<u32>(m.scale_factor());
        let x = area.position.x + (area.size.width as i32 - size.width as i32) / 2;
        let y = area.position.y + area.size.height as i32
            - size.height as i32
            - (120.0 * m.scale_factor()) as i32;
        win.set_position(PhysicalPosition::new(x, y))?;
    }
    Ok(())
}

/// Shows the reply window over the game, focused, in `target`.
pub fn open(app: &AppHandle, target: &str) -> Result<(), String> {
    let win = app
        .get_webview_window(LABEL)
        .ok_or("fenêtre de réponse absente")?;
    win.show().map_err(|e| e.to_string())?;
    win.set_always_on_top(true).map_err(|e| e.to_string())?;
    win.set_focus().map_err(|e| e.to_string())?;
    win.emit_to(LABEL, OPEN_EVENT, target)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn reply_phrases() -> Vec<phrases::Phrase> {
    phrases::PHRASES
        .iter()
        .filter(|p| p.quick)
        .copied()
        .collect()
}

#[tauri::command]
pub fn reply_phrase_text(id: String, target: String) -> Option<String> {
    let p = phrases::by_id(&id)?;
    Some(if target == "ru" { p.ru } else { p.en }.to_string())
}

#[tauri::command]
pub async fn reply_translate(
    reply: tauri::State<'_, Arc<ReplyService>>,
    text: String,
    target: String,
) -> Result<ReplyResult, String> {
    let svc = reply.inner().clone();
    tauri::async_runtime::spawn_blocking(move || svc.translate(&text, &target))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn reply_models(reply: tauri::State<'_, Arc<ReplyService>>) -> Vec<ReplyModel> {
    reply.models()
}

/// Opens the reply window (from the main window: button or Ctrl+Alt+F7).
#[tauri::command]
pub fn reply_open(app: AppHandle, recent_langs: Vec<String>) -> Result<(), String> {
    open(&app, guess_target(&recent_langs))
}

#[tauri::command]
pub fn reply_hide(app: AppHandle) -> Result<(), String> {
    let win = app
        .get_webview_window(LABEL)
        .ok_or("fenêtre de réponse absente")?;
    win.hide().map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct Echo(&'static str);
    impl Engine for Echo {
        fn translate(&mut self, text: &str) -> Result<String, String> {
            Ok(format!("{}:{text}", self.0))
        }
    }

    fn service(loads: Arc<AtomicUsize>) -> Arc<ReplyService> {
        ReplyService::with_loader(
            PathBuf::from("nowhere"),
            Box::new(move |target| {
                loads.fetch_add(1, Ordering::SeqCst);
                Ok(Box::new(Echo(if target == "ru" { "RU" } else { "EN" })) as Box<dyn Engine>)
            }),
        )
    }

    #[test]
    fn usual_calls_skip_the_model() {
        let loads = Arc::new(AtomicUsize::new(0));
        let svc = service(loads.clone());
        let r = svc.translate("Drop-moi une arme stp", "ru").unwrap();
        assert_eq!(
            (r.text.as_str(), r.via),
            ("Скинь мне оружие, пожалуйста", "phrasebook")
        );
        assert_eq!(loads.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn free_text_goes_to_the_model_once_loaded_and_keeps_cs_terms() {
        let loads = Arc::new(AtomicUsize::new(0));
        let svc = service(loads.clone());
        svc.set_map_terms(vec!["Palace".into()]);
        let r = svc.translate("attention, smoke au palace", "en").unwrap();
        assert_eq!(r.via, "model");
        assert_eq!(r.text, "EN:attention, smoke au palace");
        svc.translate("ils arrivent par le haut", "en").unwrap();
        assert_eq!(loads.load(Ordering::SeqCst), 1, "the model is loaded once");
        assert!(svc
            .translate("ils arrivent", "ru")
            .unwrap()
            .text
            .starts_with("RU:"));
        assert_eq!(loads.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn refuses_bad_input_and_reports_missing_models() {
        let svc = service(Arc::new(AtomicUsize::new(0)));
        assert!(svc.translate("   ", "en").is_err());
        assert!(svc.translate("salut", "de").is_err());
        assert!(svc.translate(&"a".repeat(301), "en").is_err());
        let real = ReplyService::new(std::env::temp_dir().join("crosstalk-reply-none"));
        let e = real
            .translate("on se retrouve après le round", "ru")
            .unwrap_err();
        assert!(e.contains("absent"), "{e}");
        assert!(real.models().iter().all(|m| !m.installed && !m.loaded));
        assert_eq!(real.models().len(), 2);
    }

    #[test]
    fn guesses_the_teammates_language() {
        let l = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(guess_target(&l(&["ru", "ru", "en", "fr"])), "ru");
        assert_eq!(guess_target(&l(&["en", "ru"])), "en");
        assert_eq!(guess_target(&[]), "en");
    }
}
