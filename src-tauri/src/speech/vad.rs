//! Voice activity detection: Silero VAD v5 (ONNX, MIT) run by sherpa-onnx.
//!
//! Silero answers "is there a human voice?" on 32 ms frames. It is robust to
//! steady noise but not to every shot or explosion; the segmenter and the
//! transcript filter add CS2-specific guards on top. sherpa-onnx exposes its
//! decision (speech or not) rather than the raw probability: each frame is
//! given as 1.0 or 0.0 to the segmenter, which keeps its own hysteresis.

use super::SpeechError;

/// Silero window: 512 samples, 32 ms at 16 kHz.
pub const FRAME: usize = 512;
pub const FRAME_MS: u32 = 32;

/// Speech probability per frame.
pub trait VoiceDetector: Send {
    /// `samples.len()` is a multiple of `FRAME`; returns one probability per frame.
    fn probabilities(&mut self, samples: &[f32]) -> Result<Vec<f32>, SpeechError>;
    /// Forgets the recurrent state (new stream, purge).
    fn reset(&mut self);
    /// Speech probability the detector itself requires (Audio & IA slider).
    fn set_threshold(&mut self, _threshold: f32) {}
}

#[cfg(feature = "asr")]
pub struct SileroVad {
    model: String,
    threshold: f32,
    vad: sherpa_onnx::VoiceActivityDetector,
}

// Used by the VAD thread only; the handle has no thread affinity.
#[cfg(feature = "asr")]
unsafe impl Send for SileroVad {}

#[cfg(feature = "asr")]
impl SileroVad {
    pub fn load(path: &std::path::Path, threshold: f32) -> Result<Self, SpeechError> {
        let model = path
            .to_str()
            .ok_or_else(|| SpeechError::Model("chemin du modèle VAD invalide".into()))?
            .to_string();
        let vad = Self::create(&model, threshold)?;
        Ok(Self {
            model,
            threshold,
            vad,
        })
    }

    fn create(
        model: &str,
        threshold: f32,
    ) -> Result<sherpa_onnx::VoiceActivityDetector, SpeechError> {
        let mut cfg = sherpa_onnx::VadModelConfig::default();
        cfg.silero_vad.model = Some(model.to_string());
        cfg.silero_vad.threshold = threshold.clamp(0.1, 0.95);
        // Raw decisions: the segmenter adds the onset and hang-over delays.
        // Not 0: the C API reads 0 as "default" (0.25 s of speech before the
        // first detection, which cut the first word of short callouts).
        cfg.silero_vad.min_silence_duration = 0.03;
        cfg.silero_vad.min_speech_duration = 0.001;
        cfg.silero_vad.window_size = FRAME as i32;
        cfg.silero_vad.max_speech_duration = 30.0;
        cfg.sample_rate = super::dsp::SAMPLE_RATE as i32;
        cfg.num_threads = 1;
        cfg.provider = Some("cpu".into());
        sherpa_onnx::VoiceActivityDetector::create(&cfg, 30.0)
            .ok_or_else(|| SpeechError::Model("VAD Silero illisible".into()))
    }
}

#[cfg(feature = "asr")]
impl VoiceDetector for SileroVad {
    fn probabilities(&mut self, samples: &[f32]) -> Result<Vec<f32>, SpeechError> {
        let probs = samples
            .chunks_exact(FRAME)
            .map(|frame| {
                self.vad.accept_waveform(frame);
                if self.vad.detected() {
                    1.0
                } else {
                    0.0
                }
            })
            .collect();
        // Segments are cut by our segmenter: drop the detector's own copies.
        self.vad.clear();
        Ok(probs)
    }

    fn reset(&mut self) {
        self.vad.reset();
    }

    fn set_threshold(&mut self, threshold: f32) {
        if (threshold - self.threshold).abs() < 1e-3 {
            return;
        }
        if let Ok(vad) = Self::create(&self.model, threshold) {
            self.vad = vad;
            self.threshold = threshold;
        }
    }
}

/// Energy-based detector used by tests and the offline bench as a reference:
/// "speech" is any frame above `threshold_db`. Not used in the app.
pub struct EnergyDetector {
    pub threshold_db: f32,
}

impl VoiceDetector for EnergyDetector {
    fn probabilities(&mut self, samples: &[f32]) -> Result<Vec<f32>, SpeechError> {
        Ok(samples
            .chunks_exact(FRAME)
            .map(|f| {
                if super::dsp::rms_db(f) > self.threshold_db {
                    1.0
                } else {
                    0.0
                }
            })
            .collect())
    }
    fn reset(&mut self) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn energy_detector_flags_loud_frames() {
        let mut d = EnergyDetector {
            threshold_db: -40.0,
        };
        let mut x = vec![0.0f32; FRAME * 3];
        x[FRAME..2 * FRAME].iter_mut().for_each(|s| *s = 0.3);
        assert_eq!(d.probabilities(&x).unwrap(), vec![0.0, 1.0, 0.0]);
        assert!(d.probabilities(&x[..100]).unwrap().is_empty());
    }
}
