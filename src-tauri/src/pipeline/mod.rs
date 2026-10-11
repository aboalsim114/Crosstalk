//! Translation pipeline: the seams for the upcoming native features.
//!
//! ```text
//!   cs2::ChatSource ───────────────────────────────┐
//!                                                  ▼
//!   audio::AudioCapture → (VAD) → asr::Transcriber → translation::Translator → UI / overlay
//! ```
//!
//! Phase 1 ships the interface with simulated data (see `src/state/store.ts`); each
//! stage here is a trait so the real implementations (WASAPI process loopback,
//! whisper-rs, DeepL/Azure, console.log tailing) can land one PR at a time and be
//! tested in isolation with fakes.

pub mod asr;
pub mod audio;
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
