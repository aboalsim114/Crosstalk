//! Speech recognition with whisper.cpp, restricted to Russian and English.

use super::filter::Recognition;
use super::types::Lang;
use super::SpeechError;

pub struct RecognizeOptions<'a> {
    pub languages: &'a [Lang],
    /// Initial prompt (glossary); empty for none.
    pub prompt: &'a str,
    pub threads: usize,
    /// See `SpeechConfig::fast_encoder`.
    pub fast_encoder: bool,
}

/// Encoder context for `samples` (Whisper: 1500 positions = 30 s), with a
/// margin of one second. `None`: full window.
pub fn audio_ctx_for(samples: usize, fast: bool) -> Option<i32> {
    if !fast {
        return None;
    }
    let secs = samples as f32 / super::dsp::SAMPLE_RATE as f32 + 1.0;
    Some(((secs * 50.0).ceil() as i32).clamp(128, 1500))
}

pub trait Recognizer: Send {
    /// Transcribes 16 kHz mono samples.
    fn recognize(
        &mut self,
        samples: &[f32],
        opts: &RecognizeOptions,
    ) -> Result<Recognition, SpeechError>;
}

/// Threads for Whisper: half of the logical CPUs, at most 4, so that CS2
/// keeps the other half (the ASR thread also runs below normal priority).
pub fn default_threads() -> usize {
    let n = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);
    (n / 2).clamp(1, 4)
}

/// whisper.cpp and ggml log every call to stderr; route them nowhere.
#[cfg(feature = "whisper")]
pub fn quiet_whisper_logs() {
    whisper_rs::install_logging_hooks();
}

/// Picks the most likely allowed language from Whisper's language probabilities.
pub fn pick_language(langs: &[Lang], prob_of: impl Fn(Lang) -> f32) -> (Lang, f32) {
    let total: f32 = langs.iter().map(|&l| prob_of(l).max(0.0)).sum();
    let best = langs
        .iter()
        .copied()
        .max_by(|a, b| prob_of(*a).total_cmp(&prob_of(*b)))
        .unwrap_or(Lang::En);
    let p = if total > 0.0 {
        prob_of(best).max(0.0) / total
    } else {
        1.0 / langs.len().max(1) as f32
    };
    (best, p)
}

#[cfg(feature = "whisper")]
pub use whisper_impl::WhisperRecognizer;

#[cfg(feature = "whisper")]
mod whisper_impl {
    use std::path::Path;

    use whisper_rs::{
        FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters, WhisperState,
    };

    use super::*;

    pub struct WhisperRecognizer {
        state: WhisperState,
        eot: i32,
    }

    impl WhisperRecognizer {
        pub fn load(path: &Path) -> Result<Self, SpeechError> {
            quiet_whisper_logs();
            let mut params = WhisperContextParameters::default();
            params.use_gpu(false);
            let ctx = WhisperContext::new_with_params(path, params)
                .map_err(|e| SpeechError::Model(format!("modèle Whisper illisible : {e}")))?;
            if !ctx.is_multilingual() {
                return Err(SpeechError::Model(
                    "modèle anglais seul : le russe exige un modèle multilingue".into(),
                ));
            }
            let state = ctx
                .create_state()
                .map_err(|e| SpeechError::Model(format!("état Whisper : {e}")))?;
            let eot = ctx.token_eot();
            Ok(Self { state, eot })
        }

        fn detect_language(
            &mut self,
            samples: &[f32],
            langs: &[Lang],
            threads: usize,
        ) -> Result<(Lang, f32), SpeechError> {
            if langs.len() == 1 {
                return Ok((langs[0], 1.0));
            }
            // whisper.cpp encodes with the window of the previous `full` call
            // here (or 30 s on the first one): at least 2.5 s, enough to tell
            // Russian from English.
            self.state
                .pcm_to_mel(samples, threads)
                .map_err(|e| SpeechError::Inference(format!("mel : {e}")))?;
            let (_, probs) = self
                .state
                .lang_detect(0, threads)
                .map_err(|e| SpeechError::Inference(format!("langue : {e}")))?;
            Ok(pick_language(langs, |l| {
                whisper_rs::get_lang_id(l.code())
                    .and_then(|id| probs.get(id as usize).copied())
                    .unwrap_or(0.0)
            }))
        }
    }

    impl Recognizer for WhisperRecognizer {
        fn recognize(
            &mut self,
            samples: &[f32],
            opts: &RecognizeOptions,
        ) -> Result<Recognition, SpeechError> {
            let threads = opts.threads.max(1);
            let (lang, lang_prob) = self.detect_language(samples, opts.languages, threads)?;

            let mut p = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
            p.set_n_threads(threads as i32);
            p.set_language(Some(lang.code()));
            p.set_translate(false);
            p.set_no_context(true);
            p.set_single_segment(true);
            p.set_no_timestamps(true);
            p.set_print_special(false);
            p.set_print_progress(false);
            p.set_print_realtime(false);
            p.set_print_timestamps(false);
            p.set_suppress_blank(true);
            // Drops "[music]", "(gunshot)"-style tokens at the source.
            p.set_suppress_nst(true);
            // No temperature fallback: predictable latency, the filter judges.
            p.set_temperature(0.0);
            p.set_temperature_inc(0.0);
            p.set_no_speech_thold(super::super::filter::NO_SPEECH_MAX);
            // Callouts are short: cap the output to stop runaway loops early.
            p.set_max_tokens(96);
            if let Some(ctx) = audio_ctx_for(samples.len(), opts.fast_encoder) {
                p.set_audio_ctx(ctx);
            }
            if !opts.prompt.is_empty() {
                p.set_initial_prompt(opts.prompt);
            }

            self.state
                .full(p, samples)
                .map_err(|e| SpeechError::Inference(format!("whisper : {e}")))?;

            let mut text = String::new();
            let (mut logprob_sum, mut tokens) = (0.0f32, 0usize);
            let mut no_speech_prob = 0.0f32;
            for i in 0..self.state.full_n_segments() {
                let Some(seg) = self.state.get_segment(i) else {
                    continue;
                };
                if i == 0 {
                    no_speech_prob = seg.no_speech_probability();
                }
                if let Ok(t) = seg.to_str_lossy() {
                    text.push_str(&t);
                }
                for j in 0..seg.n_tokens() {
                    if let Some(tok) = seg.get_token(j) {
                        let data = tok.token_data();
                        // Text tokens only: special tokens (timestamps, EOT…) come after EOT.
                        if data.id < self.eot {
                            logprob_sum += data.plog;
                            tokens += 1;
                        }
                    }
                }
            }
            Ok(Recognition {
                text: text.trim().to_string(),
                lang,
                lang_prob,
                avg_logprob: if tokens > 0 {
                    logprob_sum / tokens as f32
                } else {
                    -10.0
                },
                no_speech_prob,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picks_the_likeliest_allowed_language() {
        let probs = |l: Lang| match l {
            Lang::Ru => 0.3,
            Lang::En => 0.1,
        };
        let (l, p) = pick_language(&[Lang::Ru, Lang::En], probs);
        assert_eq!(l, Lang::Ru);
        assert!((p - 0.75).abs() < 1e-6);
        assert_eq!(pick_language(&[Lang::En], probs), (Lang::En, 1.0));
        assert_eq!(pick_language(&[Lang::Ru, Lang::En], |_| 0.0).1, 0.5);
    }

    #[test]
    fn short_segments_get_a_short_encoder_window() {
        assert_eq!(audio_ctx_for(32_000, false), None);
        assert_eq!(audio_ctx_for(32_000, true), Some(150)); // 2 s + 1 s margin
        assert_eq!(audio_ctx_for(1_000, true), Some(128));
        assert_eq!(audio_ctx_for(16_000 * 40, true), Some(1500));
    }

    #[test]
    fn leaves_cpu_for_the_game() {
        let t = default_threads();
        assert!((1..=4).contains(&t));
    }
}
