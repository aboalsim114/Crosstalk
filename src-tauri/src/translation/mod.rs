//! Local translation into French (Opus-MT, Apache-2.0), on the CPU: no API key,
//! no network, nothing to pay.

pub mod callouts;
pub mod engine;
pub mod glossary;
pub mod llm;
pub mod marian;
pub mod models;
mod network;
pub mod opus;
mod pth;
pub mod service;

pub use service::{Translation, TranslationConfig, TranslationService, TranslationStatus};
pub mod spm;

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

/// Starts the translation service with the Opus-MT models of the models
/// directory, behind the online LLM (off until the user turns it on).
pub fn start(app: &AppHandle) -> (Arc<TranslationService>, Arc<llm::LlmClient>) {
    let llm = Arc::new(llm::LlmClient::new(
        llm::default_key_store(),
        Box::new(llm::UreqTransport),
    ));
    let service = TranslationService::start(
        Arc::new(TauriEvents(app.clone())),
        Arc::new(llm::HybridLoader {
            opus: Arc::new(opus::OpusLoader),
            llm: llm.clone(),
        }),
        crate::paths::models_dir(),
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

/// Routes of the current languages (models, size, installed).
#[tauri::command]
pub fn translation_models(t: State<'_, Arc<TranslationService>>) -> Vec<models::PairInfo> {
    let c = t.status().config;
    models::list(&crate::paths::models_dir(), &c.target, &c.sources)
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

// ---------- online LLM (Audio & IA › Traduction en ligne) ----------

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
    llm.test(&target)
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

#[cfg(test)]
mod fixture_tests {
    //! Equivalence with Hugging Face transformers on `tests/fixtures/marian-tiny`
    //! (built by `scripts/make-marian-fixture.py`).
    use super::marian::{Marian, Mode};
    use super::spm::SentencePiece;
    use std::path::PathBuf;

    fn dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/marian-tiny")
    }

    fn expected() -> serde_json::Value {
        serde_json::from_slice(&std::fs::read(dir().join("expected.json")).unwrap()).unwrap()
    }

    fn strings(v: &serde_json::Value) -> Vec<String> {
        v.as_array()
            .unwrap()
            .iter()
            .map(|s| s.as_str().unwrap().to_string())
            .collect()
    }

    #[test]
    fn every_translation_pair_comes_from_a_speech_language() {
        // Kept here: models.rs is also compiled into the installer, without speech.
        for p in super::models::CATALOG {
            assert!(crate::speech::Lang::from_code(p.source).is_some());
        }
    }

    #[test]
    fn source_pieces_match_sentencepiece() {
        let sp =
            SentencePiece::from_bytes(&std::fs::read(dir().join("source.spm")).unwrap()).unwrap();
        for case in expected()["source"].as_array().unwrap() {
            let text = case["text"].as_str().unwrap();
            if let Some(norm) = case["normalized"].as_str() {
                assert_eq!(sp.normalize(text), norm, "normalisation de {text:?}");
            }
            assert_eq!(
                sp.encode(text),
                strings(&case["pieces"]),
                "pièces de {text:?}"
            );
        }
    }

    #[test]
    fn target_round_trip_matches_sentencepiece() {
        let sp =
            SentencePiece::from_bytes(&std::fs::read(dir().join("target.spm")).unwrap()).unwrap();
        for case in expected()["target"].as_array().unwrap() {
            let text = case["text"].as_str().unwrap();
            let pieces = sp.encode(text);
            assert_eq!(pieces, strings(&case["pieces"]), "pièces de {text:?}");
            let refs: Vec<&str> = pieces.iter().map(String::as_str).collect();
            assert_eq!(sp.decode(&refs), case["decoded"].as_str().unwrap());
        }
    }

    #[test]
    fn ids_generation_and_decoding_match_transformers() {
        matches_transformers(&dir());
    }

    /// Same reference outputs with the weights in PyTorch's legacy format,
    /// the format of the real Opus-MT checkpoints.
    #[test]
    fn legacy_checkpoint_matches_transformers() {
        let tmp = std::env::temp_dir().join(format!("crosstalk-legacy-{}", std::process::id()));
        std::fs::create_dir_all(&tmp).unwrap();
        for f in ["config.json", "vocab.json", "source.spm", "target.spm"] {
            std::fs::copy(dir().join(f), tmp.join(f)).unwrap();
        }
        std::fs::copy(
            dir().join("pytorch_model.legacy.bin"),
            tmp.join("pytorch_model.bin"),
        )
        .unwrap();
        matches_transformers(&tmp);
        std::fs::remove_dir_all(&tmp).unwrap();
    }

    fn matches_transformers(model_dir: &std::path::Path) {
        let mut m = Marian::load(model_dir, Mode::Exact).unwrap();
        for case in expected()["source"].as_array().unwrap() {
            let text = case["text"].as_str().unwrap();
            let ids: Vec<u32> = case["ids"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_u64().unwrap() as u32)
                .collect();
            assert_eq!(m.encode(text), ids, "ids de {text:?}");
            let Some(gen) = case["generated"].as_array() else {
                continue;
            };
            let gen: Vec<u32> = gen.iter().map(|v| v.as_u64().unwrap() as u32).collect();
            // HF forces </s> at max_new_tokens = 12: compare the 11 free steps.
            let want: Vec<u32> = gen[1..]
                .iter()
                .take(11)
                .copied()
                .take_while(|&t| t != 0)
                .collect();
            let got = m.generate(&ids, 11).unwrap();
            assert_eq!(got, want, "génération pour {text:?}");
            assert_eq!(
                m.decode(&got),
                case["decoded"].as_str().unwrap(),
                "texte pour {text:?}"
            );
        }
    }

    /// Beam search (4 beams) gives the sequences of transformers' `generate`.
    #[test]
    fn beam_search_matches_transformers() {
        let mut m = Marian::load(&dir(), Mode::Exact).unwrap();
        let mut n = 0;
        for case in expected()["source"].as_array().unwrap() {
            let Some(beam) = case["beam4"].as_array() else {
                continue;
            };
            let ids: Vec<u32> = case["ids"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_u64().unwrap() as u32)
                .collect();
            // Drop the decoder start token and the final </s>.
            let want: Vec<u32> = beam[1..]
                .iter()
                .map(|v| v.as_u64().unwrap() as u32)
                .filter(|&t| t != 0)
                .collect();
            let got = m.generate_beam(&ids, 12, 4).unwrap();
            assert_eq!(got, want, "beam search for {:?}", case["text"]);
            n += 1;
        }
        assert!(n >= 5, "{n} cases");
    }

    /// With `</s>` made likelier, hypotheses end at different lengths: the
    /// finished-hypotheses logic must match PyTorch's beam search (numpy port
    /// in scripts/make-marian-fixture.py).
    #[test]
    fn beam_search_handles_early_endings_like_pytorch() {
        let mut m = Marian::load(&dir(), Mode::Exact).unwrap();
        let exp = expected();
        m.boost_logit(0, exp["eos_boost"].as_f64().unwrap() as f32);
        let mut lengths = Vec::new();
        for case in exp["source"].as_array().unwrap() {
            let Some(want) = case["beam4_eos"].as_array() else {
                continue;
            };
            let ids: Vec<u32> = case["ids"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_u64().unwrap() as u32)
                .collect();
            let want: Vec<u32> = want
                .iter()
                .map(|v| v.as_u64().unwrap() as u32)
                .filter(|&t| t != 0)
                .collect();
            assert_eq!(
                m.generate_beam(&ids, 12, 4).unwrap(),
                want,
                "beam search for {:?}",
                case["text"]
            );
            lengths.push(want.len());
        }
        // The cases do exercise early endings, not only the length limit.
        assert!(
            lengths.iter().any(|&l| l < 11) && lengths.contains(&11),
            "{lengths:?}"
        );
    }

    #[test]
    fn fast_mode_only_generates_french_pieces_and_agrees_with_exact() {
        let mut exact = Marian::load(&dir(), Mode::Exact).unwrap();
        let mut fast = Marian::load(&dir(), Mode::Fast).unwrap();
        let target =
            SentencePiece::from_bytes(&std::fs::read(dir().join("target.spm")).unwrap()).unwrap();
        let french: std::collections::HashSet<String> =
            target.text_pieces().map(str::to_string).collect();
        let vocab: std::collections::HashMap<String, u32> =
            serde_json::from_slice(&std::fs::read(dir().join("vocab.json")).unwrap()).unwrap();
        let allowed: std::collections::HashSet<u32> = french
            .iter()
            .filter_map(|p| vocab.get(p).copied())
            .collect();
        let (mut same, mut total) = (0, 0);
        for case in expected()["source"].as_array().unwrap() {
            let text = case["text"].as_str().unwrap();
            let src = fast.encode(text);
            let got = fast.generate(&src, 11).unwrap();
            assert!(
                got.iter().all(|id| allowed.contains(id)),
                "pièce non française pour {text:?}"
            );
            let want = exact.generate(&src, 11).unwrap();
            // Where exact greedy search stays within French pieces, both agree
            // up to q8_0 rounding on near ties.
            if want.iter().all(|id| allowed.contains(id)) {
                total += 1;
                same += usize::from(got.first() == want.first());
            }
        }
        assert!(total == 0 || same * 2 >= total, "{same}/{total}");
    }
}
