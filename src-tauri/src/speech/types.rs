//! Types shared by the speech pipeline and serialised for the UI (camelCase).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Languages the recogniser writes (Parakeet TDT 0.6B v3: European languages;
/// its vocabulary has no Arabic, Chinese, Japanese or Turkish letters). The
/// language of a transcript is detected among these, never chosen by the user.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Lang {
    Ru,
    En,
    Fr,
    De,
    Es,
    Uk,
    Pl,
    Pt,
}

impl Lang {
    pub const ALL: [Lang; 8] = [
        Lang::Ru,
        Lang::En,
        Lang::Fr,
        Lang::De,
        Lang::Es,
        Lang::Uk,
        Lang::Pl,
        Lang::Pt,
    ];
    /// Every language the recogniser writes: detected, nothing to choose.
    pub const DEFAULT: [Lang; 8] = Lang::ALL;

    /// ISO 639-1 language code.
    pub fn code(self) -> &'static str {
        match self {
            Lang::Ru => "ru",
            Lang::En => "en",
            Lang::Fr => "fr",
            Lang::De => "de",
            Lang::Es => "es",
            Lang::Uk => "uk",
            Lang::Pl => "pl",
            Lang::Pt => "pt",
        }
    }

    /// Written in the Cyrillic alphabet.
    pub fn cyrillic(self) -> bool {
        matches!(self, Lang::Ru | Lang::Uk)
    }

    pub fn from_code(code: &str) -> Option<Lang> {
        Lang::ALL.into_iter().find(|l| l.code() == code)
    }
}

/// User settings of the speech pipeline (kept in memory: persistence is #15).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpeechConfig {
    /// Model id from the catalog (`models::CATALOG`).
    pub model: String,
    /// Allowed languages; never empty.
    pub languages: Vec<Lang>,
    /// Silero speech probability needed to open a segment, 0.2..0.9.
    pub vad_threshold: f32,
    /// "Focus voix": keep what is in the centre of the stereo game audio
    /// (the voice chat), turn down sounds coming from the sides (focus.rs).
    #[serde(default)]
    pub voice_focus: bool,
}

impl Default for SpeechConfig {
    fn default() -> Self {
        Self {
            model: super::models::DEFAULT_MODEL.into(),
            languages: Lang::DEFAULT.to_vec(),
            vad_threshold: 0.55,
            voice_focus: false,
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
            Lang::DEFAULT.to_vec()
        } else {
            langs
        };
        // Settings of an older version name a Whisper model.
        self.model = super::models::spec_or_default(&self.model).id.to_string();
        self
    }
}

/// One recognised utterance. This is a transcription in the speaker's
/// language: translation into French is a later feature.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Transcript {
    /// Utterance id: a partial transcript and the final one share it.
    pub id: u64,
    /// Live words of a segment still being spoken, replaced by the final one.
    pub partial: bool,
    pub text: String,
    pub lang: Lang,
    /// Confidence in `lang`, read from the text among the allowed languages.
    pub lang_prob: f32,
    /// Duration of the transcribed audio segment.
    pub audio_ms: u32,
    /// From the end of the voice to the text being available.
    pub latency_ms: u32,
    /// Recogniser compute time.
    pub asr_ms: u32,
    /// End of the voice → segment closed (the silence that ends it, the VAD hop).
    #[serde(default)]
    pub close_ms: u32,
    /// Segment closed → recognition started (queue, a live pass still running).
    #[serde(default)]
    pub queue_ms: u32,
    /// When the transcript left the core (Unix time, ms): the interface
    /// measures its own delay from it.
    #[serde(default)]
    pub emitted_at_ms: u64,
    pub model: String,
    /// Name shown on CS2's voice indicator while it was spoken (src/speaker).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speaker: Option<String>,
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
    /// The recogniser returned nothing usable.
    Empty,
    /// Known hallucination (subtitle credits, "thanks for watching"…) or too
    /// much text for the audio.
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
    /// Model folder, when every file is in place.
    pub model_path: Option<String>,
    pub vad_ready: bool,
    pub models_dir: String,
    pub error: Option<String>,
    pub load_ms: Option<u32>,
    /// Inference backend of the loaded model, e.g. "CPU · ONNX Runtime".
    pub backend: String,
    pub threads: u32,
    /// Name of the recognition model, e.g. "Parakeet TDT 0.6B v3".
    pub model_label: String,
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
    /// Recogniser compute time / audio duration of the last segment.
    pub last_rtf: Option<f32>,
    /// Recogniser compute time and audio duration of every final segment
    /// transcribed (cumulative): their ratio says if it keeps up.
    #[serde(default)]
    pub asr_ms: u64,
    #[serde(default)]
    pub asr_audio_ms: u64,
    /// CPU used by the Crosstalk core process, in % of the whole machine.
    pub cpu_percent: Option<f32>,
    /// "Focus voix": game sound removed over the last second, in dB
    /// (`None`: off, or a mono source).
    pub focus_db: Option<f32>,
    /// Where the latency goes, stage by stage (same lines as `latency_*`).
    #[serde(default)]
    pub stages: StageLatency,
}

/// p50 / p95 of one stage, in ms (`None` before the first line).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Percentiles {
    pub p50_ms: Option<u32>,
    pub p95_ms: Option<u32>,
}

/// The latency of the final lines split by stage: the segment closing after
/// the voice, the wait before recognition, the recognition itself.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StageLatency {
    pub close: Percentiles,
    pub queue: Percentiles,
    pub asr: Percentiles,
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
            voice_focus: false,
        }
        .sanitized();
        assert_eq!(c.vad_threshold, 0.9);
        assert_eq!(c.languages, vec![Lang::En]);
        // A Whisper model saved by an older version: the current model.
        assert_eq!(c.model, super::super::models::DEFAULT_MODEL);
        // Fields of older versions are ignored.
        let old: SpeechConfig = serde_json::from_value(serde_json::json!({
            "model": "small", "languages": ["ru"], "vadThreshold": 0.5,
            "glossary": ["rush B"], "accel": "auto", "fastEncoder": true
        }))
        .unwrap();
        assert_eq!(old.sanitized().model, super::super::models::DEFAULT_MODEL);
        let empty = SpeechConfig {
            languages: vec![],
            ..c
        }
        .sanitized();
        assert_eq!(empty.languages, Lang::ALL.to_vec());
    }

    #[test]
    fn transcript_serialises_for_the_ui() {
        let t = Transcript {
            id: 1,
            partial: false,
            text: "Двое на B".into(),
            lang: Lang::Ru,
            lang_prob: 0.9,
            audio_ms: 1200,
            latency_ms: 900,
            asr_ms: 400,
            close_ms: 0,
            queue_ms: 0,
            emitted_at_ms: 0,
            model: "parakeet-tdt-0.6b-v3".into(),
            speaker: None,
        };
        let v = serde_json::to_value(&t).unwrap();
        assert_eq!(v["lang"], "ru");
        assert_eq!(v["latencyMs"], 900);
        assert_eq!(v["asrMs"], 400);
        assert_eq!(
            serde_json::to_value(RejectReason::WrongScript).unwrap(),
            "wrongScript"
        );
    }
}
