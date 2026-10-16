//! Composition: the only place where one service reacts to another's events
//! (docs/architecture/decisions.md, D-03). The services themselves publish
//! through their traits (`AudioEvents`, `SpeechEvents`, `ChatEvents`) and know
//! nothing of each other.
//!
//! - a capture that stops purges speech and translation;
//! - a transcript is named (speaker), shown, then translated;
//! - a chat message is shown, its sender remembered, then translated;
//! - the user's languages reach both the translation and "Répondre".
//!
//! The routing rules are plain functions, tested without Tauri.

use std::sync::Arc;
use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager, State};

use crate::audio::{self, AudioEvents, CaptureState, CaptureStatus, LevelReading};
use crate::chat::{self, ChatEvents, ChatMessage, ChatStatus};
use crate::reply::ReplyService;
use crate::speaker::SpeakerService;
use crate::speech::{
    self, SpeechConfig, SpeechEvents, SpeechService, SpeechStats, SpeechStatus, Transcript,
    VoiceActivity,
};
use crate::translation::{languages, TranslationConfig, TranslationService, TranslationStatus};

/// Where transcripts and chat lines go to be translated.
trait Translate {
    fn submit(&self, id: u64, lang: &str, text: &str);
    fn submit_partial(&self, id: u64, lang: &str, text: &str);
}

impl Translate for TranslationService {
    fn submit(&self, id: u64, lang: &str, text: &str) {
        TranslationService::submit(self, id, lang, text);
    }
    fn submit_partial(&self, id: u64, lang: &str, text: &str) {
        TranslationService::submit_partial(self, id, lang, text);
    }
}

/// A transcript, in this order: who was talking (CS2's voice indicator,
/// opt-in), shown to the windows, then translated. Live lines get a quick
/// translation, final ones the careful one.
fn route_transcript(
    transcript: &Transcript,
    speaker: impl FnOnce(&Transcript) -> Option<String>,
    show: impl FnOnce(&Transcript),
    translation: Option<&dyn Translate>,
) {
    let named;
    let transcript = match speaker(transcript) {
        Some(name) => {
            named = Transcript {
                speaker: Some(name),
                ..transcript.clone()
            };
            &named
        }
        None => transcript,
    };
    show(transcript);
    if let Some(t) = translation {
        if transcript.partial {
            t.submit_partial(transcript.id, transcript.lang.code(), &transcript.text);
        } else {
            t.submit(transcript.id, transcript.lang.code(), &transcript.text);
        }
    }
}

/// A chat message: shown, its sender is a known name for "Qui parle ?"
/// (memory only), then translated when Crosstalk handles its language. The
/// translation service shows the user's own language and the languages not
/// chosen as is (F-32).
fn route_chat(
    message: &ChatMessage,
    show: impl FnOnce(&ChatMessage),
    sender: impl FnOnce(&str),
    translation: Option<&dyn Translate>,
) {
    show(message);
    sender(&message.sender);
    if let Some(lang) = message
        .lang
        .as_deref()
        .filter(|l| languages::is_language(l))
    {
        if let Some(t) = translation {
            t.submit(message.id, lang, &message.message);
        }
    }
}

/// Capture events go to the UI; a capture that stops also purges the speech and
/// translation pipelines (nothing survives a stop, an emergency stop or CS2 closing).
pub struct CaptureEvents {
    pub ui: audio::TauriEvents,
    pub speech: Arc<SpeechService>,
    pub translation: Arc<TranslationService>,
}

impl AudioEvents for CaptureEvents {
    fn status(&self, status: &CaptureStatus) {
        if status.state != CaptureState::Capturing {
            self.speech.purge();
            self.translation.purge();
        }
        self.ui.status(status);
    }
    fn level(&self, reading: LevelReading) {
        self.ui.level(reading);
    }
}

/// Forwards speech events to every window, and transcripts to the speaker
/// and translation services.
pub struct SpeechBridge(pub AppHandle);

impl SpeechEvents for SpeechBridge {
    fn status(&self, status: &SpeechStatus) {
        let _ = self.0.emit(speech::STATUS_EVENT, status);
    }
    fn transcript(&self, transcript: &Transcript) {
        let speaker = self.0.try_state::<Arc<SpeakerService>>();
        let translation = self.0.try_state::<Arc<TranslationService>>();
        route_transcript(
            transcript,
            |t| {
                speaker.and_then(|s| {
                    s.speaker_for(
                        Duration::from_millis(t.latency_ms as u64),
                        Duration::from_millis(t.audio_ms as u64),
                    )
                })
            },
            |t| {
                let _ = self.0.emit(speech::TRANSCRIPT_EVENT, t);
            },
            translation.as_ref().map(|t| t.as_ref() as &dyn Translate),
        );
    }
    fn activity(&self, activity: VoiceActivity) {
        if let Some(s) = self.0.try_state::<Arc<SpeakerService>>() {
            s.voice(activity.speaking);
        }
        let _ = self.0.emit(speech::ACTIVITY_EVENT, activity);
    }
    fn stats(&self, stats: &SpeechStats) {
        let _ = self.0.emit(speech::STATS_EVENT, stats);
    }
    fn retract(&self, id: u64) {
        let _ = self.0.emit(speech::RETRACT_EVENT, id);
    }
}

/// Starts the speech service with the sherpa-onnx models.
pub fn start_speech(app: &AppHandle) -> Arc<SpeechService> {
    SpeechService::start(
        Arc::new(SpeechBridge(app.clone())),
        Arc::new(speech::SherpaLoader),
        crate::paths::models_dir(),
        SpeechConfig::default(),
    )
}

/// Forwards chat events to every window, and messages to the speaker and
/// translation services.
pub struct ChatBridge(pub AppHandle);

impl ChatEvents for ChatBridge {
    fn status(&self, status: &ChatStatus) {
        let _ = self.0.emit(chat::STATUS_EVENT, status);
    }
    fn message(&self, m: &ChatMessage) {
        let speaker = self.0.try_state::<Arc<SpeakerService>>();
        let translation = self.0.try_state::<Arc<TranslationService>>();
        route_chat(
            m,
            |m| {
                let _ = self.0.emit(chat::MESSAGE_EVENT, m);
            },
            |name| {
                if let Some(s) = speaker {
                    s.chat_sender(name);
                }
            },
            translation.as_ref().map(|t| t.as_ref() as &dyn Translate),
        );
    }
}

/// The user's languages: the translation, and "Répondre", which writes from
/// the user's language into the players'.
#[tauri::command]
pub fn translation_configure(
    app: AppHandle,
    t: State<'_, Arc<TranslationService>>,
    config: TranslationConfig,
) -> TranslationStatus {
    let st = t.configure(config);
    if let Some(r) = app.try_state::<Arc<ReplyService>>() {
        r.set_languages(&st.config.target, &st.config.sources);
    }
    st
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use super::*;
    use crate::chat::parser::Channel;
    use crate::speech::Lang;

    /// Records every call, in order.
    #[derive(Default)]
    struct Log(RefCell<Vec<String>>);

    impl Log {
        fn push(&self, s: String) {
            self.0.borrow_mut().push(s);
        }
        fn take(&self) -> Vec<String> {
            self.0.take()
        }
    }

    impl Translate for Log {
        fn submit(&self, id: u64, lang: &str, text: &str) {
            self.push(format!("translate {id} {lang} {text}"));
        }
        fn submit_partial(&self, id: u64, lang: &str, text: &str) {
            self.push(format!("translate-live {id} {lang} {text}"));
        }
    }

    fn transcript(partial: bool) -> Transcript {
        Transcript {
            id: 7,
            partial,
            text: "двое на B".into(),
            lang: Lang::Ru,
            lang_prob: 1.0,
            audio_ms: 900,
            latency_ms: 400,
            asr_ms: 300,
            model: "base".into(),
            speaker: None,
        }
    }

    fn message(lang: Option<&str>) -> ChatMessage {
        ChatMessage {
            id: (1 << 40) + 3,
            channel: Channel::T,
            sender: "Nikolai".into(),
            message: "rush b".into(),
            location: None,
            dead: false,
            lang: lang.map(str::to_string),
        }
    }

    #[test]
    fn a_transcript_is_named_then_shown_then_translated() {
        let log = Log::default();
        route_transcript(
            &transcript(false),
            |t| {
                log.push(format!("speaker {} {}", t.latency_ms, t.audio_ms));
                Some("Nikolai".into())
            },
            |t| log.push(format!("show {:?} {}", t.speaker, t.text)),
            Some(&log),
        );
        assert_eq!(
            log.take(),
            [
                "speaker 400 900",
                "show Some(\"Nikolai\") двое на B",
                "translate 7 ru двое на B"
            ]
        );
    }

    #[test]
    fn a_live_line_gets_the_quick_translation() {
        let log = Log::default();
        route_transcript(
            &transcript(true),
            |_| None,
            |t| log.push(format!("show {:?}", t.speaker)),
            Some(&log),
        );
        assert_eq!(log.take(), ["show None", "translate-live 7 ru двое на B"]);
    }

    #[test]
    fn without_translation_the_transcript_is_still_shown() {
        let log = Log::default();
        route_transcript(
            &transcript(false),
            |_| None,
            |_| log.push("show".into()),
            None,
        );
        assert_eq!(log.take(), ["show"]);
    }

    #[test]
    fn a_chat_message_is_shown_then_its_sender_kept_then_translated() {
        let log = Log::default();
        route_chat(
            &message(Some("ru")),
            |m| log.push(format!("show {}", m.message)),
            |name| log.push(format!("sender {name}")),
            Some(&log),
        );
        assert_eq!(
            log.take(),
            [
                "show rush b",
                "sender Nikolai",
                &format!("translate {} ru rush b", (1u64 << 40) + 3)
            ]
        );
    }

    #[test]
    fn a_chat_message_in_an_unknown_language_is_only_shown() {
        for lang in [None, Some("it")] {
            let log = Log::default();
            route_chat(
                &message(lang),
                |_| log.push("show".into()),
                |_| log.push("sender".into()),
                Some(&log),
            );
            assert_eq!(log.take(), ["show", "sender"]);
        }
    }
}
