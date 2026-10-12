//! Translation pipeline: the seams for the upcoming native features.
//!
//! ```text
//!   cs2::ChatSource ─────────────────────────────────────────┐
//!                                                            ▼
//!   crate::audio (capture) → crate::speech (VAD, Whisper) → translation::Translator → UI / overlay
//! ```
//!
//! Capture and speech recognition are implemented in their own modules; the
//! remaining stages are traits so the real implementations (DeepL/Azure,
//! console.log tailing) can land one PR at a time and be tested with fakes.

pub mod cs2;
pub mod translation;

use serde::Serialize;

/// A line of speech or chat, as shown in the Script feed and the overlay.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Utterance {
    pub kind: UtteranceKind,
    /// ISO 639-1 source language, e.g. `ru`.
    pub lang: String,
    pub original: String,
    pub sender: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum UtteranceKind {
    Voice,
    Chat,
}

#[derive(Debug, thiserror::Error)]
pub enum PipelineError {
    #[error("{0} n’est pas encore disponible dans cette version")]
    NotImplemented(&'static str),
    #[error("périphérique audio : {0}")]
    Audio(String),
    #[error("moteur de traduction : {0}")]
    Translation(String),
}
