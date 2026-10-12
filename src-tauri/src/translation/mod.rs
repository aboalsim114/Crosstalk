//! Local translation into French (Opus-MT, Apache-2.0), on the CPU: no API key,
//! no network, nothing to pay.

pub mod glossary;
pub mod marian;
pub mod models;
mod network;
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

/// Starts the translation service with the Opus-MT models of the models directory.
pub fn start(app: &AppHandle) -> Arc<TranslationService> {
    TranslationService::start(
        Arc::new(TauriEvents(app.clone())),
        Arc::new(service::OpusLoader),
        crate::speech::models::models_dir(),
        TranslationConfig::default(),
    )
}

// ---------- commands ----------

#[tauri::command]
pub fn translation_status(t: State<'_, Arc<TranslationService>>) -> TranslationStatus {
    t.status()
}

#[tauri::command]
pub fn translation_configure(
    t: State<'_, Arc<TranslationService>>,
    config: TranslationConfig,
) -> TranslationStatus {
    t.configure(config)
}

#[tauri::command]
pub fn translation_rescan(t: State<'_, Arc<TranslationService>>) -> TranslationStatus {
    t.rescan()
}

#[tauri::command]
pub fn translation_models() -> Vec<models::PairInfo> {
    models::list(&crate::speech::models::models_dir())
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
        let mut m = Marian::load(&dir(), Mode::Exact).unwrap();
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
