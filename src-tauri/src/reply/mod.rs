//! "Réponds dans leur langue": the user types (or dictates) in their own
//! language (Langues › Ma langue); Crosstalk translates into the teammates'
//! language with the same LLM as every other translation, and the window
//! copies it to the clipboard, ready to paste in CS2's chat (Y / U, Ctrl+V).
//! Nothing is typed into the game: the user pastes and sends it themselves.
//!
//! Usual calls come from a hand-written phrasebook (`phrases.rs`, French into
//! English or Russian, exact and instant, no API call); any other sentence
//! goes to the LLM (Audio & IA › Traduction IA). Without a connected LLM, the
//! reason is shown: there is no other engine.

pub mod phrases;

use std::sync::{Arc, Mutex};
use std::time::Instant;

use serde::Serialize;
use tauri::{
    AppHandle, Emitter, LogicalSize, Manager, PhysicalPosition, WebviewUrl, WebviewWindowBuilder,
};

use crate::translation::engine::Translator;
use crate::translation::languages;

pub const LABEL: &str = "reply";
pub const OPEN_EVENT: &str = "reply://open";
const MAX_CHARS: usize = 300;
const WIDTH: f64 = 560.0;
const HEIGHT: f64 = 340.0;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReplyResult {
    pub text: String,
    pub target: String,
    /// `phrasebook` (exact, hand-written) or `llm`.
    pub via: &'static str,
    pub ms: u32,
}

/// A language the reply can be written in, and whether free text works now.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ReplyTarget {
    /// The user's language.
    pub source: String,
    pub target: String,
    /// The LLM is connected: any sentence can be translated.
    pub ready: bool,
    /// Why free text is unavailable (the phrasebook still works).
    pub problem: Option<String>,
}

/// The user's language and the players' (from the translation settings).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Languages {
    pub me: String,
    pub players: Vec<String>,
}

impl Default for Languages {
    fn default() -> Self {
        Self {
            me: languages::DEFAULT_TARGET.into(),
            players: languages::DEFAULT_SOURCES
                .iter()
                .map(|s| s.to_string())
                .collect(),
        }
    }
}

pub struct ReplyService {
    translator: Arc<dyn Translator>,
    langs: Mutex<Languages>,
}

impl ReplyService {
    pub fn new(translator: Arc<dyn Translator>) -> Arc<Self> {
        Arc::new(Self {
            translator,
            langs: Mutex::new(Languages::default()),
        })
    }

    /// Follows Langues › Ma langue and the players' languages.
    pub fn set_languages(&self, me: &str, players: &[String]) {
        *self.langs.lock().unwrap() = Languages {
            me: me.to_string(),
            players: players.to_vec(),
        };
    }

    pub fn languages(&self) -> Languages {
        self.langs.lock().unwrap().clone()
    }

    /// Languages the reply can be written in.
    pub fn targets(&self) -> Vec<String> {
        let l = self.languages();
        languages::reply_targets(&l.me, &l.players)
    }

    pub fn models(&self) -> Vec<ReplyTarget> {
        let me = self.languages().me;
        let problem = self.translator.problem();
        self.targets()
            .into_iter()
            .map(|target| ReplyTarget {
                source: me.clone(),
                target,
                ready: problem.is_none(),
                problem: problem.clone(),
            })
            .collect()
    }

    /// Kept for the map changes (the LLM keeps map callouts by its instructions).
    pub fn set_map_terms(&self, _callouts: Vec<String>) {}

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
        if let Some(p) = self.translator.problem() {
            return Err(p);
        }
        let out = self.translator.translate(text, &me, target)?;
        let out = crate::translation::callouts::clean_output(&out);
        if out.is_empty() {
            return Err("traduction vide".into());
        }
        Ok(done(out, self.translator.name()))
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
pub fn reply_models(reply: tauri::State<'_, Arc<ReplyService>>) -> Vec<ReplyTarget> {
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

    /// "<TARGET>:<text>" like an LLM would translate; can be disconnected.
    struct FakeLlm {
        calls: AtomicUsize,
        connected: bool,
    }
    impl Translator for FakeLlm {
        fn problem(&self) -> Option<String> {
            (!self.connected)
                .then(|| "aucun moteur de traduction connecté (Audio & IA › Traduction IA)".into())
        }
        fn translate(&self, text: &str, source: &str, target: &str) -> Result<String, String> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(format!(
                "{}>{}:{text}",
                source.to_uppercase(),
                target.to_uppercase()
            ))
        }
        fn translate_partial(&self, _: &str, _: &str, _: &str) -> Result<String, String> {
            Err("non".into())
        }
    }

    fn service(connected: bool) -> (Arc<ReplyService>, Arc<FakeLlm>) {
        let llm = Arc::new(FakeLlm {
            calls: AtomicUsize::new(0),
            connected,
        });
        (ReplyService::new(llm.clone()), llm)
    }

    #[test]
    fn usual_calls_skip_the_llm() {
        let (svc, llm) = service(true);
        let r = svc.translate("Drop-moi une arme stp", "ru").unwrap();
        assert_eq!(
            (r.text.as_str(), r.via),
            ("Скинь мне оружие, пожалуйста", "phrasebook")
        );
        assert_eq!(llm.calls.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn any_sentence_goes_to_the_llm_from_the_users_language() {
        let (svc, llm) = service(true);
        let r = svc
            .translate("Mon équipe ne sait pas jouer.", "ru")
            .unwrap();
        assert_eq!(
            (r.via, r.text.as_str()),
            ("llm", "FR>RU:Mon équipe ne sait pas jouer.")
        );
        assert_eq!(
            svc.translate("ils arrivent par le haut", "en")
                .unwrap()
                .text,
            "FR>EN:ils arrivent par le haut"
        );
        assert_eq!(llm.calls.load(Ordering::SeqCst), 2);
        assert!(svc.models().iter().all(|m| m.ready && m.problem.is_none()));
    }

    #[test]
    fn without_a_connected_llm_free_text_says_why_but_phrases_still_work() {
        let (svc, llm) = service(false);
        let e = svc
            .translate("on se retrouve après le round", "ru")
            .unwrap_err();
        assert!(e.contains("aucun moteur"), "{e}");
        assert_eq!(svc.translate("éco", "en").unwrap().via, "phrasebook");
        assert_eq!(llm.calls.load(Ordering::SeqCst), 0);
        let m = svc.models();
        assert_eq!(m.len(), 2);
        assert!(m.iter().all(|t| !t.ready && t.problem.is_some()));
    }

    #[test]
    fn refuses_bad_input() {
        let (svc, _) = service(true);
        assert!(svc.translate("   ", "en").is_err());
        assert!(svc.translate("salut", "de").is_err());
        assert!(svc.translate(&"a".repeat(301), "en").is_err());
    }

    #[test]
    fn a_russian_user_replies_from_russian_into_every_players_language() {
        let (svc, _) = service(true);
        let l = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        svc.set_languages("ru", &l(&["en", "de", "pl", "ru"]));
        // Polish too: the LLM writes every language; their own: nothing to do.
        assert_eq!(svc.targets(), ["en", "de", "pl"]);
        // The French phrasebook does not apply to Russian input.
        let r = svc.translate("gg", "en").unwrap();
        assert_eq!((r.via, r.text.as_str()), ("llm", "RU>EN:gg"));
        assert_eq!(
            svc.translate("иду на Б", "pl").unwrap().text,
            "RU>PL:иду на Б"
        );
        assert!(svc.translate("да", "ru").is_err());
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
