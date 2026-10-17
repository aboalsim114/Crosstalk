//! Translation into the user's language by the LLM they chose (cloud with
//! their own key, or a local server such as Ollama or LM Studio). Whisper
//! turns speech into text; the LLM translates it. No local translation model
//! is shipped or downloaded by Crosstalk.

pub mod callouts;
pub mod engine;
pub mod glossary;
pub mod guard;
pub mod languages;
pub mod legacy;
pub mod llm;
pub mod service;

pub use service::{Translation, TranslationConfig, TranslationService, TranslationStatus};

use std::sync::Arc;

use tauri::{AppHandle, Emitter, State};

pub const STATUS_EVENT: &str = "translation://status";
pub const RESULT_EVENT: &str = "translation://result";

/// Forwards translation events to every window.
pub struct TauriEvents(pub AppHandle);

impl service::TranslationEvents for TauriEvents {
    fn status(&self, status: &TranslationStatus) {
        let _ = self.0.emit(STATUS_EVENT, status);
    }
    fn result(&self, translation: &Translation) {
        let _ = self.0.emit(RESULT_EVENT, translation);
    }
}

/// Starts the translation service on the LLM client (not connected until the
/// user configures it).
pub fn start(app: &AppHandle) -> (Arc<TranslationService>, Arc<llm::LlmClient>) {
    let llm = Arc::new(llm::LlmClient::new(
        llm::default_key_store(),
        Box::new(llm::UreqTransport),
    ));
    let service = TranslationService::start(
        Arc::new(TauriEvents(app.clone())),
        llm.clone(),
        TranslationConfig::default(),
    );
    (service, llm)
}

// ---------- commands ----------

#[tauri::command]
pub fn translation_status(t: State<'_, Arc<TranslationService>>) -> TranslationStatus {
    t.status()
}

#[tauri::command]
pub fn translation_rescan(t: State<'_, Arc<TranslationService>>) -> TranslationStatus {
    t.rescan()
}

/// Manual test from the Audio page: blocks a worker thread, not the UI.
#[tauri::command(async)]
pub fn translation_test(
    t: State<'_, Arc<TranslationService>>,
    lang: String,
    text: String,
) -> Translation {
    t.translate_blocking(&lang, &text)
}

/// Opus-MT models left by earlier versions (size shown in Audio & IA).
#[tauri::command(async)]
pub fn translation_legacy_models() -> legacy::LegacyModels {
    legacy::scan(&crate::paths::models_dir())
}

/// Removes them, on the user's request only.
#[tauri::command(async)]
pub fn translation_legacy_cleanup() -> Result<legacy::LegacyModels, String> {
    let left = legacy::remove(&crate::paths::models_dir())?;
    crate::logs::info(
        "traduction",
        "anciens modèles Opus-MT supprimés à la demande de l'utilisateur",
    );
    Ok(left)
}

// ---------- the LLM (Audio & IA › Traduction IA) ----------

/// Settings and counters; tells whether a key is stored, never the key.
#[tauri::command]
pub fn llm_status(llm: State<'_, Arc<llm::LlmClient>>) -> llm::LlmStatus {
    llm.status()
}

#[tauri::command]
pub fn llm_configure(
    llm: State<'_, Arc<llm::LlmClient>>,
    t: State<'_, Arc<TranslationService>>,
    config: llm::LlmConfig,
) -> llm::LlmStatus {
    let before = llm.status();
    let st = llm.configure(config);
    if before.config != st.config || before.active != st.active {
        t.engines_changed();
        // A local model is loaded in the background before the first line.
        llm.warm_up();
    }
    st
}

/// Stores the key (Windows Credential Manager); it never comes back to the UI.
#[tauri::command]
pub fn llm_set_key(
    llm: State<'_, Arc<llm::LlmClient>>,
    t: State<'_, Arc<TranslationService>>,
    key: String,
) -> Result<llm::LlmStatus, String> {
    let st = llm.set_key(&key)?;
    t.engines_changed();
    crate::logs::info("traduction", "clé API du LLM enregistrée");
    Ok(st)
}

#[tauri::command]
pub fn llm_clear_key(
    llm: State<'_, Arc<llm::LlmClient>>,
    t: State<'_, Arc<TranslationService>>,
) -> Result<llm::LlmStatus, String> {
    let st = llm.clear_key()?;
    t.engines_changed();
    crate::logs::info("traduction", "clé API du LLM effacée");
    Ok(st)
}

/// "Tester la connexion": translates a sample sentence into the user's
/// language with the current settings. Async: never on the main thread.
#[tauri::command(async)]
pub fn llm_test(
    llm: State<'_, Arc<llm::LlmClient>>,
    t: State<'_, Arc<TranslationService>>,
) -> llm::LlmTest {
    let target = t.status().config.target;
    let source = if target == "ru" { "en" } else { "ru" };
    let text = if source == "en" {
        "Two enemies on B, one is low."
    } else {
        llm::TEST_SENTENCE
    };
    llm.test(text, source, &target)
}

/// "Tester une traduction": the user's own sentence, between any two languages.
#[tauri::command(async)]
pub fn llm_translate_test(
    llm: State<'_, Arc<llm::LlmClient>>,
    text: String,
    source: String,
    target: String,
) -> Result<llm::LlmTest, String> {
    let text = text.trim();
    if text.is_empty() || text.chars().count() > 300 {
        return Err("phrase vide ou trop longue (300 caractères au plus)".into());
    }
    if !languages::is_language(&source) || !languages::is_language(&target) {
        return Err("langue non gérée".into());
    }
    Ok(llm.test(text, &source, &target))
}

/// Models of the configured server: the provider's list (with the stored
/// key) or the models installed on a local server.
#[tauri::command(async)]
pub fn llm_models(llm: State<'_, Arc<llm::LlmClient>>) -> llm::LlmModels {
    llm.list_models()
}

/// Ollama and LM Studio on this PC, with their models (nothing is installed).
#[tauri::command(async)]
pub fn llm_local_servers() -> Vec<llm::LocalServer> {
    llm::detect_local()
}

/// Ollama running on this PC and its installed models (no key needed).
#[tauri::command(async)]
pub fn llm_ollama_local() -> llm::OllamaLocal {
    llm::detect_ollama()
}

/// Opens the page where a service creates API keys (fixed list, see llm::KEY_PAGES).
#[tauri::command(async)]
pub fn llm_open_key_page(service: String) -> Result<(), String> {
    let url = llm::key_page(&service).ok_or("service inconnu")?;
    crate::logs::shell_open(url)
}
