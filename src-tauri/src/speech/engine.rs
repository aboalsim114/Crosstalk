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

/// Text tokens Whisper may produce for `samples` (16 kHz): fast Russian is
/// about 9 tokens per second, so 12 per second plus a margin never cuts real
/// speech, while a looping decode stops after a short segment's worth.
pub fn max_tokens_for(samples: usize) -> i32 {
    let secs = samples as f32 / super::dsp::SAMPLE_RATE as f32;
    ((16.0 + 12.0 * secs).ceil() as i32).clamp(24, 96)
}

/// Sampling temperature of the second pass after a looping greedy decode
/// (the first fallback step of OpenAI's reference decoder).
pub const RETRY_TEMPERATURE: f32 = 0.2;

#[derive(Debug, Default, Clone, Copy)]
pub struct InferenceTiming {
    pub language_ms: u32,
    pub full_ms: u32,
}

pub trait Recognizer: Send {
    /// Timing of the previous call, if supported by the backend.
    fn timing(&self) -> Option<InferenceTiming> {
        None
    }

    /// Where it runs, e.g. "CPU · AVX2" or "GPU · <name> (Vulkan)".
    fn backend(&self) -> Option<String> {
        None
    }

    /// Transcribes 16 kHz mono samples in a given language (the user's own
    /// dictated reply, in French): plain text, no filtering.
    fn transcribe_in(
        &mut self,
        samples: &[f32],
        lang: &str,
        prompt: &str,
        threads: usize,
    ) -> Result<String, SpeechError> {
        let _ = (samples, lang, prompt, threads);
        Err(SpeechError::Model("dictée non prise en charge".into()))
    }

    /// Transcribes 16 kHz mono samples.
    fn recognize(
        &mut self,
        samples: &[f32],
        opts: &RecognizeOptions,
    ) -> Result<Recognition, SpeechError>;
}

/// The GPU is kept in Auto mode only if it does the test in at most this
/// share of the CPU's time: it also draws CS2, a small gain is not worth it.
pub const AUTO_GPU_MAX_RATIO: f32 = 0.75;

/// Auto mode: `(use the GPU, CPU time / GPU time)`.
pub fn pick_accel(gpu_ms: u32, cpu_ms: u32) -> (bool, f32) {
    let speedup = cpu_ms.max(1) as f32 / gpu_ms.max(1) as f32;
    (gpu_ms as f32 <= cpu_ms as f32 * AUTO_GPU_MAX_RATIO, speedup)
}

/// "4,2×" (French decimal comma).
pub fn speedup_label(x: f32) -> String {
    format!("{x:.1}×").replace('.', ",")
}

/// Threads for Whisper: half of the logical CPUs, at most 4, so that CS2
/// keeps the other half (the ASR thread also runs below normal priority).
pub fn logical_threads() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
}

pub fn sanitize_threads(threads: usize) -> usize {
    threads.clamp(1, logical_threads())
}

pub fn default_threads() -> usize {
    let n = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);
    (n / 2).clamp(1, 4)
}

/// whisper.cpp and ggml log every call to stderr; route them nowhere.
#[cfg(feature = "whisper")]
/// `CROSSTALK_WHISPER_LOGS=1` keeps them on stderr (diagnosis).
pub fn quiet_whisper_logs() {
    if std::env::var_os("CROSSTALK_WHISPER_LOGS").is_some_and(|v| v == "1") {
        return;
    }
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
pub use whisper_impl::{vulkan_gpu, WhisperRecognizer};

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
        timing: InferenceTiming,
        backend: String,
    }

    // Through ggml's device registry: its Vulkan registration catches the
    // errors of a missing or too old driver (calling the Vulkan backend
    // directly would let them escape and abort the process).
    #[cfg(feature = "gpu")]
    extern "C" {
        fn ggml_backend_dev_count() -> usize;
        fn ggml_backend_dev_get(index: usize) -> *mut std::ffi::c_void;
        fn ggml_backend_dev_type(device: *mut std::ffi::c_void) -> std::os::raw::c_int;
        fn ggml_backend_dev_description(
            device: *mut std::ffi::c_void,
        ) -> *const std::os::raw::c_char;
    }

    /// Name of the first GPU ggml can use (Vulkan driver present and recent
    /// enough), if any.
    #[cfg(feature = "gpu")]
    pub fn vulkan_gpu() -> Option<String> {
        const GPU: std::os::raw::c_int = 1;
        const IGPU: std::os::raw::c_int = 2;
        // SAFETY: ggml's C API; devices live for the whole process and the
        // description is a NUL-terminated string owned by ggml.
        unsafe {
            (0..ggml_backend_dev_count()).find_map(|i| {
                let dev = ggml_backend_dev_get(i);
                let kind = ggml_backend_dev_type(dev);
                if dev.is_null() || (kind != GPU && kind != IGPU) {
                    return None;
                }
                let desc = ggml_backend_dev_description(dev);
                let name = if desc.is_null() {
                    String::new()
                } else {
                    std::ffi::CStr::from_ptr(desc)
                        .to_string_lossy()
                        .trim()
                        .to_string()
                };
                Some(if name.is_empty() {
                    "carte graphique".into()
                } else {
                    name
                })
            })
        }
    }

    #[cfg(not(feature = "gpu"))]
    pub fn vulkan_gpu() -> Option<String> {
        None
    }

    impl WhisperRecognizer {
        /// `gpu`: on the graphics card when this build has Vulkan and the PC
        /// a usable GPU; otherwise on the CPU.
        pub fn load(path: &Path, gpu: bool) -> Result<Self, SpeechError> {
            quiet_whisper_logs();
            let device = if gpu { vulkan_gpu() } else { None };
            let mut params = WhisperContextParameters::default();
            params.use_gpu(device.is_some());
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
            let backend = match device {
                Some(name) => format!("GPU · {name} (Vulkan)"),
                None => "CPU · AVX2".into(),
            };
            let mut recognizer = Self {
                state,
                eot,
                timing: InferenceTiming::default(),
                backend,
            };
            recognizer.warm_up(default_threads())?;
            Ok(recognizer)
        }

        /// Time to transcribe 3 s of test audio (low noise, a few tokens):
        /// mostly the encoder, which dominates a callout.
        pub fn bench_ms(&mut self) -> Result<u32, SpeechError> {
            let n = crate::speech::dsp::SAMPLE_RATE as usize * 3;
            let mut seed = 0x2545_f491_u32;
            let audio: Vec<f32> = (0..n)
                .map(|_| {
                    seed ^= seed << 13;
                    seed ^= seed >> 17;
                    seed ^= seed << 5;
                    (seed as f32 / u32::MAX as f32 - 0.5) * 0.02
                })
                .collect();
            let mut p = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
            p.set_n_threads(default_threads() as i32);
            p.set_language(Some("ru"));
            p.set_no_context(true);
            p.set_single_segment(true);
            p.set_no_timestamps(true);
            p.set_print_special(false);
            p.set_print_progress(false);
            p.set_print_realtime(false);
            p.set_print_timestamps(false);
            p.set_max_tokens(8);
            if let Some(ctx) = audio_ctx_for(audio.len(), true) {
                p.set_audio_ctx(ctx);
            }
            let started = std::time::Instant::now();
            self.state
                .full(p, &audio)
                .map_err(|e| SpeechError::Model(format!("mesure Whisper : {e}")))?;
            Ok(started.elapsed().as_millis() as u32)
        }

        pub fn set_backend(&mut self, backend: String) {
            self.backend = backend;
        }

        pub fn backend_name(&self) -> &str {
            &self.backend
        }

        /// One short pass on silence while the model loads. whisper.cpp's
        /// language detection encodes with the window of the previous `full`
        /// call, the whole 30 s before any: without this, the first callout of
        /// a session spent seconds detecting its language (measured 5 to 8 s
        /// in a CS2 game) and was dropped as stale.
        fn warm_up(&mut self, threads: usize) -> Result<(), SpeechError> {
            let silence = vec![0.0f32; crate::speech::dsp::SAMPLE_RATE as usize];
            let mut p = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
            p.set_n_threads(threads as i32);
            p.set_language(Some("en"));
            p.set_no_context(true);
            p.set_single_segment(true);
            p.set_no_timestamps(true);
            p.set_print_special(false);
            p.set_print_progress(false);
            p.set_print_realtime(false);
            p.set_print_timestamps(false);
            p.set_max_tokens(1);
            if let Some(ctx) = audio_ctx_for(silence.len(), true) {
                p.set_audio_ctx(ctx);
            }
            self.state
                .full(p, &silence)
                .map_err(|e| SpeechError::Model(format!("préchauffage Whisper : {e}")))
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
        fn timing(&self) -> Option<InferenceTiming> {
            Some(self.timing)
        }
        fn backend(&self) -> Option<String> {
            Some(self.backend.clone())
        }
        fn transcribe_in(
            &mut self,
            samples: &[f32],
            lang: &str,
            prompt: &str,
            threads: usize,
        ) -> Result<String, SpeechError> {
            let mut p = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
            p.set_n_threads(threads.max(1) as i32);
            p.set_language(Some(lang));
            p.set_translate(false);
            p.set_no_context(true);
            p.set_single_segment(true);
            p.set_no_timestamps(true);
            p.set_print_special(false);
            p.set_print_progress(false);
            p.set_print_realtime(false);
            p.set_print_timestamps(false);
            p.set_suppress_blank(true);
            p.set_suppress_nst(true);
            p.set_temperature(0.0);
            p.set_max_tokens(128);
            if let Some(ctx) = audio_ctx_for(samples.len(), true) {
                p.set_audio_ctx(ctx);
            }
            if !prompt.is_empty() {
                p.set_initial_prompt(prompt);
            }
            self.state
                .full(p, samples)
                .map_err(|e| SpeechError::Inference(format!("whisper : {e}")))?;
            let mut text = String::new();
            for i in 0..self.state.full_n_segments() {
                if let Some(seg) = self.state.get_segment(i) {
                    if let Ok(t) = seg.to_str_lossy() {
                        text.push_str(&t);
                    }
                }
            }
            Ok(text.trim().to_string())
        }
        fn recognize(
            &mut self,
            samples: &[f32],
            opts: &RecognizeOptions,
        ) -> Result<Recognition, SpeechError> {
            let threads = opts.threads.max(1);
            self.timing = InferenceTiming::default();
            let language_started = std::time::Instant::now();
            let (lang, lang_prob) = self.detect_language(samples, opts.languages, threads)?;
            self.timing.language_ms = language_started.elapsed().as_millis() as u32;

            let p = self.params(lang, threads, samples.len(), opts);
            let full_started = std::time::Instant::now();
            let mut rec = self.decode(p, samples, lang, lang_prob)?;
            // Greedy decoding can lock into a loop. When the loop is a whole
            // sentence repeated, the filter keeps that sentence for free
            // (`collapse_repeats`). Otherwise ("they are rotating mid, they
            // are rotating mid, …" cut short), decode once more with sampling,
            // the first fallback step of OpenAI's reference decoder. A retry
            // over the full 30 s window was measured too slow on the CPU
            // (+4 s, later lines then went stale).
            use super::super::filter::{clean, collapse_repeats, is_looping};
            if is_looping(&rec.text) && collapse_repeats(&clean(&rec.text)).is_none() {
                let mut p = self.params(lang, threads, samples.len(), opts);
                p.set_temperature(RETRY_TEMPERATURE);
                let retry = self.decode(p, samples, lang, lang_prob)?;
                if !is_looping(&retry.text) {
                    rec = retry;
                }
            }
            self.timing.full_ms = full_started.elapsed().as_millis() as u32;
            Ok(rec)
        }
    }

    impl WhisperRecognizer {
        fn params<'a>(
            &self,
            lang: Lang,
            threads: usize,
            samples: usize,
            opts: &'a RecognizeOptions,
        ) -> FullParams<'a, 'a> {
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
            // Callouts are short: cap the output to what the segment can hold,
            // which also stops a runaway loop early.
            p.set_max_tokens(max_tokens_for(samples));
            if let Some(ctx) = audio_ctx_for(samples, opts.fast_encoder) {
                p.set_audio_ctx(ctx);
            }
            if !opts.prompt.is_empty() {
                p.set_initial_prompt(opts.prompt);
            }
            p
        }

        fn decode(
            &mut self,
            p: FullParams,
            samples: &[f32],
            lang: Lang,
            lang_prob: f32,
        ) -> Result<Recognition, SpeechError> {
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
    fn auto_keeps_the_gpu_only_when_clearly_faster() {
        assert_eq!(pick_accel(250, 1000), (true, 4.0));
        assert!(pick_accel(750, 1000).0);
        assert!(
            !pick_accel(800, 1000).0,
            "a 20 % gain is not worth the game's GPU"
        );
        assert!(!pick_accel(1200, 1000).0);
        assert_eq!(speedup_label(4.04), "4,0×");
    }

    #[test]
    fn picks_the_likeliest_allowed_language() {
        let probs = |l: Lang| match l {
            Lang::Ru => 0.3,
            Lang::En => 0.1,
            _ => 0.0,
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
    fn the_token_budget_follows_the_segment_length() {
        assert_eq!(max_tokens_for(16_000), 28); // 1 s
        assert_eq!(max_tokens_for(26_560), 36); // 1.66 s (a loop seen on a 3-word callout)
        assert_eq!(max_tokens_for(800), 24); // floor
        assert_eq!(max_tokens_for(16_000 * 8), 96); // longest segment: the former cap
    }

    #[test]
    fn leaves_cpu_for_the_game() {
        let t = default_threads();
        assert!((1..=4).contains(&t));
    }
    #[test]
    fn explicit_threads_never_oversubscribe() {
        assert_eq!(sanitize_threads(0), 1);
        assert_eq!(sanitize_threads(usize::MAX), logical_threads());
    }
}
