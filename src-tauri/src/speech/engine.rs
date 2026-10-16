//! Speech recognition with sherpa-onnx (ONNX Runtime, CPU): NVIDIA Parakeet
//! TDT 0.6B v3 by default (`models::CATALOG`). The model writes the text in
//! the language it hears (25 languages, punctuation included); the language
//! is then read from the text, among the languages the user allows.

use super::filter::Recognition;
use super::types::Lang;
use super::SpeechError;

pub struct RecognizeOptions<'a> {
    /// Languages the transcript may be in (the players' and the user's).
    pub languages: &'a [Lang],
}

pub trait Recognizer: Send {
    /// Where it runs, e.g. "CPU · ONNX Runtime".
    fn backend(&self) -> Option<String> {
        None
    }

    /// Transcribes 16 kHz mono samples (the user's own dictated reply): plain
    /// text, no filtering. `lang` is the user's language, a hint only: the
    /// model detects the language by itself.
    fn transcribe_in(&mut self, samples: &[f32], lang: &str) -> Result<String, SpeechError> {
        let _ = lang;
        self.recognize(samples, &RecognizeOptions { languages: &[] })
            .map(|r| r.text)
    }

    /// Transcribes 16 kHz mono samples.
    fn recognize(
        &mut self,
        samples: &[f32],
        opts: &RecognizeOptions,
    ) -> Result<Recognition, SpeechError>;
}

pub fn logical_threads() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
}

pub fn sanitize_threads(threads: usize) -> usize {
    threads.clamp(1, logical_threads())
}

/// Threads for the recogniser: half of the logical CPUs, at most 4, so that
/// CS2 keeps the other half.
pub fn default_threads() -> usize {
    (logical_threads() / 2).clamp(1, 4)
}

/// The language of a transcript, among `allowed` (empty: any of Crosstalk's
/// languages), with a confidence: read from the text, since the model gives
/// none. `None`: the text gives no usable clue (no letters).
pub fn identify(text: &str, allowed: &[Lang]) -> Option<(Lang, f32)> {
    let allowed: &[Lang] = if allowed.is_empty() {
        &Lang::ALL
    } else {
        allowed
    };
    let letters: Vec<char> = text.chars().filter(|c| c.is_alphabetic()).collect();
    if letters.is_empty() {
        return None;
    }
    let cyrillic = letters.iter().filter(|c| is_cyrillic(**c)).count() * 2 >= letters.len();
    if let Some(l) = crate::chat::lang::detect(text).and_then(Lang::from_code) {
        if allowed.contains(&l) {
            return Some((l, 0.9));
        }
    }
    // No clear answer: the first allowed language written in the same script.
    allowed
        .iter()
        .copied()
        .find(|l| l.cyrillic() == cyrillic && (*l == Lang::En || cyrillic))
        .or_else(|| allowed.iter().copied().find(|l| l.cyrillic() == cyrillic))
        .map(|l| (l, 0.5))
        .or(Some((allowed[0], 0.2)))
}

fn is_cyrillic(c: char) -> bool {
    ('\u{0400}'..='\u{04FF}').contains(&c)
}

#[cfg(feature = "asr")]
pub use sherpa_impl::SherpaRecognizer;

#[cfg(feature = "asr")]
mod sherpa_impl {
    use std::path::Path;

    use sherpa_onnx::{OfflineRecognizer, OfflineRecognizerConfig};

    use super::*;
    use crate::speech::models::{ModelKind, ModelSpec};

    pub struct SherpaRecognizer {
        recognizer: OfflineRecognizer,
        backend: String,
    }

    // The recogniser is used by one thread at a time (behind the service's
    // mutex); sherpa-onnx's handle has no thread affinity.
    unsafe impl Send for SherpaRecognizer {}

    fn path(dir: &Path, name: &str) -> Option<String> {
        dir.join(name).to_str().map(str::to_string)
    }

    impl SherpaRecognizer {
        pub fn load(spec: &ModelSpec, dir: &Path, threads: usize) -> Result<Self, SpeechError> {
            let mut cfg = OfflineRecognizerConfig::default();
            match spec.kind {
                ModelKind::NemoTransducer => {
                    let file = |prefix: &str| {
                        spec.files
                            .iter()
                            .find(|f| f.name.starts_with(prefix))
                            .and_then(|f| path(dir, f.name))
                    };
                    cfg.model_config.transducer.encoder = file("encoder");
                    cfg.model_config.transducer.decoder = file("decoder");
                    cfg.model_config.transducer.joiner = file("joiner");
                    cfg.model_config.tokens = file("tokens");
                    cfg.model_config.model_type = Some("nemo_transducer".into());
                }
            }
            cfg.model_config.num_threads = sanitize_threads(threads) as i32;
            cfg.model_config.provider = Some("cpu".into());
            cfg.decoding_method = Some("greedy_search".into());
            let recognizer = OfflineRecognizer::create(&cfg).ok_or_else(|| {
                SpeechError::Model(format!("{} illisible (fichiers du modèle)", spec.label))
            })?;
            let mut me = Self {
                recognizer,
                backend: "CPU · ONNX Runtime".into(),
            };
            // First call allocates the inference buffers: done now, not on the
            // first callout of the match.
            let _ = me.decode(&vec![0.0; 16_000]);
            Ok(me)
        }

        fn decode(&mut self, samples: &[f32]) -> Result<String, SpeechError> {
            let stream = self.recognizer.create_stream();
            stream.accept_waveform(crate::speech::dsp::SAMPLE_RATE as i32, samples);
            self.recognizer.decode(&stream);
            let result = stream
                .get_result()
                .ok_or_else(|| SpeechError::Inference("aucun résultat".into()))?;
            Ok(result.text.trim().to_string())
        }
    }

    impl Recognizer for SherpaRecognizer {
        fn backend(&self) -> Option<String> {
            Some(self.backend.clone())
        }

        fn recognize(
            &mut self,
            samples: &[f32],
            opts: &RecognizeOptions,
        ) -> Result<Recognition, SpeechError> {
            let text = self.decode(samples)?;
            let (lang, lang_prob) = identify(&text, opts.languages)
                .unwrap_or((opts.languages.first().copied().unwrap_or(Lang::En), 0.0));
            Ok(Recognition {
                text,
                lang,
                lang_prob,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_language_from_the_text_among_the_allowed_ones() {
        let both = [Lang::Ru, Lang::En];
        assert_eq!(identify("Двое на B.", &both).unwrap().0, Lang::Ru);
        assert_eq!(identify("He's behind you", &both), Some((Lang::En, 0.9)));
        // No telling word: the allowed Latin language, English first.
        assert_eq!(identify("Rush B", &both).unwrap().0, Lang::En);
        assert_eq!(
            identify("Rush B", &[Lang::Ru, Lang::Fr]).unwrap().0,
            Lang::Fr
        );
        // French dictation.
        assert_eq!(
            identify("Il est derrière toi", &[Lang::Fr, Lang::Ru])
                .unwrap()
                .0,
            Lang::Fr
        );
        // Ukrainian letters.
        assert_eq!(
            identify("Їх двоє", &[Lang::Ru, Lang::Uk]).unwrap().0,
            Lang::Uk
        );
        // Cyrillic text while only Latin languages are allowed: low confidence.
        assert_eq!(identify("Двое", &[Lang::En]), Some((Lang::En, 0.2)));
        assert_eq!(identify("...", &both), None);
        // Any language when none is given (dictation).
        assert_eq!(identify("Двое на B", &[]).unwrap().0, Lang::Ru);
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
