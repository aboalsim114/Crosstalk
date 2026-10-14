//! "Réponds dans leur langue": the user types in their own language (Langues ›
//! Ma langue), Crosstalk translates into the teammates' language on the CPU
//! and the window copies it to the clipboard, ready to paste in CS2's chat
//! (Y / U, Ctrl+V). Nothing is typed into the game: the user pastes and sends
//! it themselves.
//!
//! Usual calls come from a hand-written phrasebook (`phrases.rs`, French into
//! English or Russian, exact and instant); free text goes to Opus-MT
//! (Apache-2.0, through English when there is no direct model), loaded the
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

use crate::translation::engine::Engine;
use crate::translation::glossary::{self, Term};
use crate::translation::models;

pub const LABEL: &str = "reply";
pub const OPEN_EVENT: &str = "reply://open";
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
    /// The user's language.
    pub source: String,
    pub target: String,
    /// Repositories of the route ("A + B" through English).
    pub repo: String,
    pub size_mb: u32,
    pub installed: bool,
    pub loaded: bool,
}

/// Loads the engine `from` → `to`.
pub type Loader = Box<dyn Fn(&str, &str) -> Result<Box<dyn Engine>, String> + Send + Sync>;

/// The user's language and the players' (from the translation settings).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Languages {
    pub me: String,
    pub players: Vec<String>,
}

impl Default for Languages {
    fn default() -> Self {
        Self {
            me: models::DEFAULT_TARGET.into(),
            players: models::DEFAULT_SOURCES
                .iter()
                .map(|s| s.to_string())
                .collect(),
        }
    }
}

pub struct ReplyService {
    models_dir: PathBuf,
    loader: Loader,
    engines: Mutex<HashMap<String, Box<dyn Engine>>>,
    map_terms: Mutex<Vec<String>>,
    langs: Mutex<Languages>,
}

impl ReplyService {
    pub fn new(models_dir: PathBuf) -> Arc<Self> {
        let dir = models_dir.clone();
        Self::with_loader(
            models_dir,
            Box::new(move |from, to| {
                use crate::translation::{engine::EngineLoader, opus::OpusLoader};
                let route = models::route(from, to).ok_or("langue non gérée")?;
                if let Some((p, f)) = route
                    .iter()
                    .find_map(|p| models::missing_file(&dir, p).map(|f| (p, f)))
                {
                    return Err(format!("modèle {} absent ({f}) : téléchargez-le", p.dir));
                }
                OpusLoader.load(&dir, from, to)
            }),
        )
    }

    pub fn with_loader(models_dir: PathBuf, loader: Loader) -> Arc<Self> {
        Arc::new(Self {
            models_dir,
            loader,
            engines: Mutex::new(HashMap::new()),
            map_terms: Mutex::new(Vec::new()),
            langs: Mutex::new(Languages::default()),
        })
    }

    /// Follows Langues › Ma langue and the players' languages.
    pub fn set_languages(&self, me: &str, players: &[String]) {
        let next = Languages {
            me: me.to_string(),
            players: players.to_vec(),
        };
        let mut l = self.langs.lock().unwrap();
        if *l != next {
            *l = next;
            // Engines from the old language are no use any more.
            self.engines.lock().unwrap().clear();
        }
    }

    pub fn languages(&self) -> Languages {
        self.langs.lock().unwrap().clone()
    }

    /// Languages the reply can be written in.
    pub fn targets(&self) -> Vec<String> {
        let l = self.languages();
        models::reply_targets(&l.me, &l.players)
    }

    pub fn models(&self) -> Vec<ReplyModel> {
        let me = self.languages().me;
        let engines = self.engines.lock().unwrap();
        self.targets()
            .into_iter()
            .filter_map(|t| {
                let r = models::route(&me, &t)?;
                Some(ReplyModel {
                    source: me.clone(),
                    repo: r.iter().map(|p| p.repo).collect::<Vec<_>>().join(" + "),
                    size_mb: models::size_mb(&r),
                    installed: models::route_installed(&self.models_dir, &me, &t),
                    loaded: engines.contains_key(&t),
                    target: t,
                })
            })
            .collect()
    }

    /// Callouts of the map being played, kept as typed.
    pub fn set_map_terms(&self, callouts: Vec<String>) {
        *self.map_terms.lock().unwrap() = callouts;
    }

    pub fn translate(&self, text: &str, target: &str) -> Result<ReplyResult, String> {
        let started = Instant::now();
        let me = self.languages().me;
        if !self.targets().iter().any(|t| t == target) {
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
        // Hand-written phrases: French into English or Russian.
        if me == "fr" && (target == "en" || target == "ru") {
            if let Some(t) = phrases::lookup(text, target) {
                return Ok(done(t, "phrasebook"));
            }
        }
        let mut engines = self.engines.lock().unwrap();
        if !engines.contains_key(target) {
            let e = (self.loader)(&me, target)?;
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

/// Teammates' language: the most frequent reply language among recent
/// lines (the first one in a tie, English when nothing tells).
pub fn guess_target(recent_langs: &[String], targets: &[String]) -> String {
    let mut best: Option<(&String, usize)> = None;
    for t in targets {
        let n = recent_langs.iter().filter(|l| *l == t).count();
        if n > 0 && best.map_or(true, |b| n > b.1) {
            best = Some((t, n));
        }
    }
    best.map(|b| b.0.clone())
        .or_else(|| targets.iter().find(|t| *t == "en").cloned())
        .or_else(|| targets.first().cloned())
        .unwrap_or_else(|| "en".into())
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
pub fn reply_open(
    app: AppHandle,
    reply: tauri::State<'_, Arc<ReplyService>>,
    recent_langs: Vec<String>,
) -> Result<(), String> {
    open(&app, &guess_target(&recent_langs, &reply.targets()))
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
            Box::new(move |_from, target| {
                loads.fetch_add(1, Ordering::SeqCst);
                Ok(Box::new(Echo(match target {
                    "ru" => "RU",
                    "de" => "DE",
                    _ => "EN",
                })) as Box<dyn Engine>)
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
    fn a_russian_user_replies_from_russian() {
        let loads = Arc::new(AtomicUsize::new(0));
        let svc = service(loads.clone());
        let l = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        svc.set_languages("ru", &l(&["en", "de", "pl"]));
        // Polish: no model writes into it; their own language: nothing to do.
        assert_eq!(svc.targets(), ["en", "de"]);
        // The French phrasebook does not apply to Russian input.
        let r = svc.translate("gg", "en").unwrap();
        assert_eq!((r.via, r.text.as_str()), ("model", "EN:gg"));
        assert_eq!(svc.translate("иду на Б", "de").unwrap().text, "DE:иду на Б");
        assert!(svc.translate("да", "ru").is_err());
        assert!(svc.translate("да", "pl").is_err());
        let m = svc.models();
        assert_eq!(m.len(), 2);
        assert_eq!(
            m[1].repo,
            "Helsinki-NLP/opus-mt-ru-en + Helsinki-NLP/opus-mt-en-de"
        );
    }

    #[test]
    fn guesses_the_teammates_language() {
        let l = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        let t = l(&["ru", "en"]);
        assert_eq!(guess_target(&l(&["ru", "ru", "en", "fr"]), &t), "ru");
        assert_eq!(guess_target(&l(&["en", "ru"]), &t), "ru");
        assert_eq!(guess_target(&[], &t), "en");
        assert_eq!(
            guess_target(&l(&["de", "de", "en"]), &l(&["en", "de"])),
            "de"
        );
        assert_eq!(guess_target(&[], &l(&["de"])), "de");
    }
}
