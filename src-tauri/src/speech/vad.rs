//! Voice activity detection: Silero VAD v5 through its ggml port in whisper.cpp
//! (same network as the ONNX release, no ONNX Runtime to ship).
//!
//! Silero answers "is there a human voice?" on 32 ms frames. It is robust to
//! steady noise but not to every shot or explosion; the segmenter and the
//! transcript filter add CS2-specific guards on top.

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
}

/// whisper.cpp resets the Silero LSTM on every call, so each call is given
/// this much preceding audio to warm the state up (cost: about 3x the frames).
#[cfg(feature = "whisper")]
const CONTEXT_FRAMES: usize = 12;

#[cfg(feature = "whisper")]
pub struct SileroVad {
    ctx: whisper_rs::WhisperVadContext,
    context: Vec<f32>,
    window: Vec<f32>,
}

#[cfg(feature = "whisper")]
impl SileroVad {
    pub fn load(path: &std::path::Path) -> Result<Self, SpeechError> {
        super::engine::quiet_whisper_logs();
        let mut params = whisper_rs::WhisperVadContextParams::new();
        params.set_n_threads(1);
        params.set_use_gpu(false);
        let path_str = path
            .to_str()
            .ok_or_else(|| SpeechError::Model("chemin du modèle VAD invalide".into()))?;
        let ctx = whisper_rs::WhisperVadContext::new(path_str, params)
            .map_err(|e| SpeechError::Model(format!("VAD Silero illisible : {e}")))?;
        Ok(Self {
            ctx,
            context: Vec::new(),
            window: Vec::new(),
        })
    }
}

#[cfg(feature = "whisper")]
impl VoiceDetector for SileroVad {
    fn probabilities(&mut self, samples: &[f32]) -> Result<Vec<f32>, SpeechError> {
        let frames = samples.len() / FRAME;
        if frames == 0 {
            return Ok(Vec::new());
        }
        self.window.clear();
        self.window.extend_from_slice(&self.context);
        self.window.extend_from_slice(&samples[..frames * FRAME]);
        self.ctx
            .detect_speech(&self.window)
            .map_err(|e| SpeechError::Inference(format!("VAD : {e}")))?;
        let probs = self.ctx.probabilities();
        let fresh = probs[probs.len().saturating_sub(frames)..].to_vec();

        let keep = (CONTEXT_FRAMES * FRAME).min(self.window.len());
        self.context.clear();
        self.context
            .extend_from_slice(&self.window[self.window.len() - keep..]);
        Ok(fresh)
    }

    fn reset(&mut self) {
        self.context.clear();
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
