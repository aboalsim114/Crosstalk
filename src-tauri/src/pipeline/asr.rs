//! Speech recognition (to be implemented with whisper.cpp via whisper-rs).

use super::{audio::AudioFrame, PipelineError};

pub struct Transcript {
    pub text: String,
    pub lang: String,
    /// Streaming partial result, replaced by a final one.
    pub provisional: bool,
}

pub trait Transcriber: Send {
    /// `hints` biases decoding toward the CS glossary (smoke, rush B, eco…).
    fn transcribe(
        &mut self,
        segment: &[AudioFrame],
        hints: &[String],
    ) -> Result<Transcript, PipelineError>;
}
