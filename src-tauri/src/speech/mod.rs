//! Speech recognition of the captured CS2 audio, in the players' languages.
//!
//! 16 kHz conditioning → Silero VAD → segmentation with shot/explosion guards
//! → sherpa-onnx recogniser (Parakeet TDT 0.6B v3) → hallucination filter →
//! `speech://transcript` events. All local, on the CPU. `crate::wiring` then hands each
//! accepted transcript to the translation service (`translation://result`).

pub mod bench;
pub mod dsp;
pub mod engine;
pub mod filter;
pub mod focus;
pub mod legacy;
pub mod models;
pub mod segmenter;
pub mod service;
pub mod types;
pub mod vad;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use tauri::State;

pub use service::{EngineLoader, SpeechEvents, SpeechService};
pub use types::*;

pub const STATUS_EVENT: &str = "speech://status";
pub const TRANSCRIPT_EVENT: &str = "speech://transcript";
pub const ACTIVITY_EVENT: &str = "speech://activity";
pub const STATS_EVENT: &str = "speech://stats";
pub const RETRACT_EVENT: &str = "speech://retract";

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum SpeechError {
    #[error("modèle non installé : {0}")]
    NotInstalled(String),
    #[error("{0}")]
    Model(String),
    #[error("{0}")]
    Inference(String),
}

/// Loads the sherpa-onnx models (Silero VAD, the recogniser) from the models directory.
pub struct SherpaLoader;

impl EngineLoader for SherpaLoader {
    fn load_detector(&self, dir: &Path) -> Result<Box<dyn vad::VoiceDetector>, SpeechError> {
        let path = models::vad_path(dir)
            .ok_or_else(|| SpeechError::NotInstalled(models::VAD_FILE.into()))?;
        #[cfg(feature = "asr")]
        {
            Ok(Box::new(vad::SileroVad::load(&path, 0.5)?))
        }
        #[cfg(not(feature = "asr"))]
        {
            let _ = path;
            Err(SpeechError::Model(
                "build sans moteur de reconnaissance".into(),
            ))
        }
    }

    fn load_recognizer(
        &self,
        dir: &Path,
        model: &str,
        threads: usize,
    ) -> Result<Box<dyn engine::Recognizer>, SpeechError> {
        let spec = models::spec_or_default(model);
        let path = models::model_path(dir, spec.id)
            .ok_or_else(|| SpeechError::NotInstalled(spec.label.into()))?;
        #[cfg(feature = "asr")]
        {
            Ok(Box::new(engine::SherpaRecognizer::load(
                spec, &path, threads,
            )?))
        }
        #[cfg(not(feature = "asr"))]
        {
            let _ = (path, threads);
            Err(SpeechError::Model(
                "build sans moteur de reconnaissance".into(),
            ))
        }
    }

    fn model_path(&self, dir: &Path, model: &str) -> Option<PathBuf> {
        models::model_path(dir, models::spec_or_default(model).id)
    }

    fn backend(&self) -> String {
        "CPU · ONNX Runtime".into()
    }
}

// ---------- commands ----------

#[tauri::command]
pub fn speech_status(speech: State<'_, Arc<SpeechService>>) -> SpeechStatus {
    speech.status()
}

#[tauri::command]
pub fn speech_stats(speech: State<'_, Arc<SpeechService>>) -> SpeechStats {
    speech.stats()
}

#[tauri::command]
pub fn speech_models(speech: State<'_, Arc<SpeechService>>) -> Vec<models::ModelInfo> {
    models::list(speech.models_dir())
}

#[tauri::command]
pub fn speech_configure(
    speech: State<'_, Arc<SpeechService>>,
    config: SpeechConfig,
) -> SpeechStatus {
    speech.configure(config)
}

/// Looks for newly installed model files.
#[tauri::command]
pub fn speech_rescan(speech: State<'_, Arc<SpeechService>>) -> SpeechStatus {
    speech.rescan()
}

/// Whisper files left by earlier versions (size shown in Audio & IA).
#[tauri::command(async)]
pub fn speech_legacy_whisper(speech: State<'_, Arc<SpeechService>>) -> legacy::LegacyWhisper {
    legacy::scan(speech.models_dir())
}

/// Removes them, on the user's request only.
#[tauri::command(async)]
pub fn speech_legacy_cleanup(
    speech: State<'_, Arc<SpeechService>>,
) -> Result<legacy::LegacyWhisper, String> {
    let left = legacy::remove(speech.models_dir())?;
    crate::logs::info(
        "asr",
        "anciens fichiers Whisper supprimés à la demande de l'utilisateur",
    );
    Ok(left)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_files_are_reported_as_not_installed() {
        let dir = std::env::temp_dir().join("crosstalk-no-models");
        assert!(matches!(
            SherpaLoader.load_detector(&dir),
            Err(SpeechError::NotInstalled(_))
        ));
        assert!(matches!(
            SherpaLoader.load_recognizer(&dir, models::DEFAULT_MODEL, 2),
            Err(SpeechError::NotInstalled(_))
        ));
    }
}
