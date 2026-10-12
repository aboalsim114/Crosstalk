//! Types shared by the speech pipeline and serialised for the UI (camelCase).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Languages the recogniser may output. Restricting Whisper to these avoids
/// mis-detections (Ukrainian, Bulgarian…) on short or noisy CS2 callouts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Lang {
    Ru,
    En,
}

impl Lang {
    pub const ALL: [Lang; 2] = [Lang::Ru, Lang::En];

    /// Whisper language code.
    pub fn code(self) -> &'static str {
        match self {
            Lang::Ru => "ru",
            Lang::En => "en",
        }
    }

    pub fn from_code(code: &str) -> Option<Lang> {
        Lang::ALL.into_iter().find(|l| l.code() == code)
    }
}

/// User settings of the speech pipeline (kept in memory: persistence is #15).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpeechConfig {
    /// Model id from the catalog (`tiny`, `base`, `small`…).
    pub model: String,
    /// Allowed languages; never empty.
    pub languages: Vec<Lang>,
    /// Silero speech probability needed to open a segment, 0.2..0.9.
    pub vad_threshold: f32,
    /// CS2 vocabulary given to Whisper as initial prompt.
    pub glossary: Vec<String>,
    /// Encode only the segment's duration instead of Whisper's fixed 30 s
    /// window: several times faster, possibly less accurate. Off until the
    /// bench says otherwise; not exposed in the UI.
    #[serde(default)]
    pub fast_encoder: bool,
}

impl Default for SpeechConfig {
    fn default() -> Self {
        Self {
            model: super::models::DEFAULT_MODEL.into(),
            languages: Lang::ALL.to_vec(),
            vad_threshold: 0.55,
            glossary: super::filter::DEFAULT_GLOSSARY
                .iter()
                .map(|s| s.to_string())
                .collect(),
            fast_encoder: false,
        }
    }
}

impl SpeechConfig {
    /// Clamps the values coming from the UI to safe ranges.
    pub fn sanitized(mut self) -> Self {
        self.vad_threshold = self.vad_threshold.clamp(0.2, 0.9);
        let mut langs: Vec<Lang> = Vec::new();
        for l in self.languages {
            if !langs.contains(&l) {
                langs.push(l);
            }
        }
        self.languages = if langs.is_empty() {
            Lang::ALL.to_vec()
        } else {
            langs
        };
        self.glossary = self
            .glossary
            .into_iter()
            .map(|t| t.replace('\0', "").trim().to_string())
            .filter(|t| !t.is_empty())
            .take(64)
            .collect();
        self
    }
}

/// One recognised utterance. This is a transcription in the speaker's
/// language: translation into French is a later feature.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Transcript {
    pub id: u64,
    pub text: String,
    pub lang: Lang,
    /// Probability of `lang` among the allowed languages (1 when only one is allowed).
    pub lang_prob: f32,
    /// Duration of the transcribed audio segment.
    pub audio_ms: u32,
    /// From the end of the voice to the text being available.
    pub latency_ms: u32,
    /// Whisper compute time.
    pub asr_ms: u32,
    pub avg_logprob: f32,
    pub no_speech_prob: f32,
    pub model: String,
}

/// Why a segment did not produce a transcript.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RejectReason {
    /// Too little voiced audio (clicks, footsteps).
    Short,
    /// Impulsive sound: a shot or an explosion opened the segment.
    Transient,
    /// Waited too long in the queue: the line is no longer relevant.
    Stale,
    /// The queue was full: the oldest segment was dropped.
    Overflow,
    /// Whisper returned nothing usable.
    Empty,
    /// Whisper judged the segment as non-speech.
    NoSpeech,
    /// Average token log-probability too low.
    LowConfidence,
    /// Known Whisper hallucination (subtitle credits, "thanks for watching"…).
    Hallucination,
    /// Looping output.
    Repetition,
    /// Text in a script that does not match the detected language.
    WrongScript,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum EngineState {
    /// The model or the VAD file is not installed.
    Missing,
    Loading,
    Ready,
    Error,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpeechStatus {
    pub state: EngineState,
    pub config: SpeechConfig,
    /// Whisper model file, when found.
    pub model_path: Option<String>,
    pub vad_ready: bool,
    pub models_dir: String,
    pub error: Option<String>,
    pub load_ms: Option<u32>,
    /// Inference backend, e.g. "CPU · AVX2". No GPU in this build.
    pub backend: String,
    pub threads: u32,
}

/// Rolling counters for the UI and the diagnostic page.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpeechStats {
    pub segments: u64,
    pub transcribed: u64,
    pub rejected: BTreeMap<RejectReason, u64>,
    /// Audio blocks dropped because the pipeline could not keep up.
    pub dropped_blocks: u64,
    pub latency_p50_ms: Option<u32>,
    pub latency_p95_ms: Option<u32>,
    /// Whisper compute time / audio duration of the last segment.
    pub last_rtf: Option<f32>,
    /// CPU used by the Crosstalk core process, in % of the whole machine.
    pub cpu_percent: Option<f32>,
}

/// Live voice activity, emitted when it changes.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceActivity {
    pub speaking: bool,
    /// Stage being worked on: "vad" (listening) or "asr" (transcribing).
    pub transcribing: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_is_clamped_and_deduplicated() {
        let c = SpeechConfig {
            model: "base".into(),
            languages: vec![Lang::En, Lang::En],
            vad_threshold: 2.0,
            glossary: vec!["  rush B ".into(), "".into(), "a\0wp".into()],
            fast_encoder: false,
        }
        .sanitized();
        assert_eq!(c.vad_threshold, 0.9);
        assert_eq!(c.languages, vec![Lang::En]);
        assert_eq!(c.glossary, vec!["rush B", "awp"]);
        let empty = SpeechConfig {
            languages: vec![],
            ..c
        }
        .sanitized();
        assert_eq!(empty.languages, vec![Lang::Ru, Lang::En]);
    }

    #[test]
    fn transcript_serialises_for_the_ui() {
        let t = Transcript {
            id: 1,
            text: "Двое на B".into(),
            lang: Lang::Ru,
            lang_prob: 0.9,
            audio_ms: 1200,
            latency_ms: 900,
            asr_ms: 400,
            avg_logprob: -0.3,
            no_speech_prob: 0.01,
            model: "base".into(),
        };
        let v = serde_json::to_value(&t).unwrap();
        assert_eq!(v["lang"], "ru");
        assert_eq!(v["latencyMs"], 900);
        assert_eq!(v["noSpeechProb"].as_f64().unwrap() as f32, 0.01);
        assert_eq!(
            serde_json::to_value(RejectReason::NoSpeech).unwrap(),
            "noSpeech"
        );
    }
}
