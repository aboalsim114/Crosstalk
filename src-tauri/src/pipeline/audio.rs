//! Audio handed to speech recognition.
//!
//! Capture itself (WASAPI process loopback, device fallback, levels) lives in
//! `crate::audio`; this stage will receive its blocks through a `BlockSink`,
//! resampled to the format below.

/// Mono 16 kHz PCM frames, the format whisper expects.
pub struct AudioFrame {
    pub samples: Vec<f32>,
}
