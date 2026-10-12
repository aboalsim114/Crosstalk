//! Speech recognition of the captured CS2 audio (Russian, English).
//!
//! 16 kHz conditioning → Silero VAD → segmentation with shot/explosion guards
//! → whisper.cpp → hallucination filter → `speech://transcript` events. All
//! local, on the CPU, below normal priority. Each accepted transcript is then
//! translated into French by the translation service (`translation://result`).

pub mod bench;
pub mod cpu;
pub mod dsp;
pub mod engine;
pub mod filter;
pub mod models;
pub mod segmenter;
pub mod service;
pub mod types;
pub mod vad;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use tauri::{AppHandle, Emitter, Manager, State};

pub use service::{EngineLoader, SpeechEvents, SpeechService};
pub use types::*;

pub const STATUS_EVENT: &str = "speech://status";
pub const TRANSCRIPT_EVENT: &str = "speech://transcript";
pub const ACTIVITY_EVENT: &str = "speech://activity";
pub const STATS_EVENT: &str = "speech://stats";

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum SpeechError {
    #[error("modèle non installé : {0}")]
    NotInstalled(String),
    #[error("{0}")]
    Model(String),
    #[error("{0}")]
    Inference(String),
}

/// Loads the whisper.cpp models from the models directory.
pub struct WhisperLoader;

impl EngineLoader for WhisperLoader {
    fn load_detector(&self, dir: &Path) -> Result<Box<dyn vad::VoiceDetector>, SpeechError> {
        let path = models::vad_path(dir)
            .ok_or_else(|| SpeechError::NotInstalled(models::VAD_FILE.into()))?;
        #[cfg(feature = "whisper")]
        {
            Ok(Box::new(vad::SileroVad::load(&path)?))
        }
        #[cfg(not(feature = "whisper"))]
        {
            let _ = path;
            Err(SpeechError::Model("build sans whisper.cpp".into()))
        }
    }

    fn load_recognizer(
        &self,
        dir: &Path,
        model: &str,
    ) -> Result<Box<dyn engine::Recognizer>, SpeechError> {
        let path = models::model_path(dir, model)
            .ok_or_else(|| SpeechError::NotInstalled(model.into()))?;
        #[cfg(feature = "whisper")]
        {
            Ok(Box::new(engine::WhisperRecognizer::load(&path)?))
        }
        #[cfg(not(feature = "whisper"))]
        {
            let _ = path;
            Err(SpeechError::Model("build sans whisper.cpp".into()))
        }
    }

    fn model_path(&self, dir: &Path, model: &str) -> Option<PathBuf> {
        models::model_path(dir, model)
    }

    fn backend(&self) -> String {
        "CPU · AVX2".into()
    }
}

/// Forwards speech events to every window.
pub struct TauriEvents(pub AppHandle);

impl SpeechEvents for TauriEvents {
    fn status(&self, status: &SpeechStatus) {
        let _ = self.0.emit(STATUS_EVENT, status);
    }
    fn transcript(&self, transcript: &Transcript) {
        let _ = self.0.emit(TRANSCRIPT_EVENT, transcript);
        if let Some(t) = self
            .0
            .try_state::<Arc<crate::translation::TranslationService>>()
        {
            t.submit(transcript.id, transcript.lang.code(), &transcript.text);
        }
    }
    fn activity(&self, activity: VoiceActivity) {
        let _ = self.0.emit(ACTIVITY_EVENT, activity);
    }
    fn stats(&self, stats: &SpeechStats) {
        let _ = self.0.emit(STATS_EVENT, stats);
    }
}

/// Starts the speech service with the whisper.cpp models.
pub fn start(app: &AppHandle) -> Arc<SpeechService> {
    SpeechService::start(
        Arc::new(TauriEvents(app.clone())),
        Arc::new(WhisperLoader),
        models::models_dir(),
        SpeechConfig::default(),
    )
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_files_are_reported_as_not_installed() {
        let dir = std::env::temp_dir().join("crosstalk-no-models");
        assert!(matches!(
            WhisperLoader.load_detector(&dir),
            Err(SpeechError::NotInstalled(_))
        ));
        assert!(matches!(
            WhisperLoader.load_recognizer(&dir, "base"),
            Err(SpeechError::NotInstalled(_))
        ));
    }
}
