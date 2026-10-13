//! Spoken reply: the user presses Ctrl+Alt+F6 (or "Dicter"), says a sentence
//! in French; Crosstalk records the default microphone until the end of the
//! sentence (or a second press), transcribes it with Whisper, translates it to
//! the teammates' language (src/reply) and copies it to the clipboard, ready to
//! paste in CS2's chat. The microphone is only open while the user dictates
//! (F-22, F-50); the audio stays in memory and is dropped afterwards.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::audio::backend::Packet;
use crate::speech::dsp::{rms_db, Conditioner, SAMPLE_RATE};

pub const EVENT: &str = "dictation://state";
/// Longest dictation; a callout reply is a sentence.
pub const MAX_SECONDS: f32 = 12.0;
/// Without any speech after this, the dictation stops.
const NO_SPEECH_SECONDS: f32 = 5.0;
/// Silence that ends the sentence once speech was heard.
const END_SILENCE_SECONDS: f32 = 1.1;
/// 20 ms frames at 16 kHz.
const FRAME: usize = SAMPLE_RATE as usize / 50;
const SPEECH_DB: f32 = -40.0;
const SILENCE_DB: f32 = -47.0;
/// Consecutive loud frames that count as speech (60 ms).
const SPEECH_FRAMES: u32 = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndReason {
    /// Silence after the sentence.
    Silence,
    /// Nothing was said.
    NoSpeech,
    /// Too long.
    MaxLength,
}

/// Finds the end of a dictated sentence from 20 ms frame levels (the
/// microphone is close and clean: an energy detector is enough).
#[derive(Debug, Default)]
pub struct Endpointer {
    frames: u32,
    loud_run: u32,
    quiet_run: u32,
    heard: bool,
}

impl Endpointer {
    pub fn heard_speech(&self) -> bool {
        self.heard
    }

    /// One 20 ms frame; `Some` when the dictation should stop.
    pub fn frame(&mut self, db: f32) -> Option<EndReason> {
        self.frames += 1;
        let secs = self.frames as f32 / 50.0;
        if db > SPEECH_DB {
            self.loud_run += 1;
            if self.loud_run >= SPEECH_FRAMES {
                self.heard = true;
            }
        } else {
            self.loud_run = 0;
        }
        if db < SILENCE_DB {
            self.quiet_run += 1;
        } else {
            self.quiet_run = 0;
        }
        if secs >= MAX_SECONDS {
            return Some(EndReason::MaxLength);
        }
        if !self.heard && secs >= NO_SPEECH_SECONDS {
            return Some(EndReason::NoSpeech);
        }
        if self.heard && self.quiet_run as f32 / 50.0 >= END_SILENCE_SECONDS {
            return Some(EndReason::Silence);
        }
        None
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DictationState {
    /// `listening`, `transcribing`, `done` or `error`.
    pub state: &'static str,
    /// Microphone level, 0..1 (listening).
    pub level: f32,
    pub seconds: f32,
    /// What was understood, in French.
    pub text: Option<String>,
    /// The translation, copied to the clipboard.
    pub translated: Option<String>,
    pub target: String,
    /// `phrasebook` or `model`.
    pub via: Option<&'static str>,
    pub copied: bool,
    pub error: Option<String>,
}

#[derive(Default)]
pub struct Dictation {
    running: AtomicBool,
    stop: Arc<AtomicBool>,
    last: Mutex<Option<DictationState>>,
}

impl Dictation {
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    /// Asks a running dictation to stop (it then transcribes what it has).
    pub fn finish(&self) {
        self.stop.store(true, Ordering::SeqCst);
    }
}

fn emit(app: &AppHandle, st: &DictationState) {
    if let Some(d) = app.try_state::<Arc<Dictation>>() {
        *d.last.lock().unwrap() = Some(st.clone());
    }
    let _ = app.emit(EVENT, st);
}

/// Starts a dictation, or finishes the running one.
pub fn toggle(app: &AppHandle, target: &'static str) {
    let Some(d) = app.try_state::<Arc<Dictation>>().map(|d| d.inner().clone()) else {
        return;
    };
    if d.running.swap(true, Ordering::SeqCst) {
        d.finish();
        return;
    }
    d.stop.store(false, Ordering::SeqCst);
    let app = app.clone();
    std::thread::Builder::new()
        .name("dictation".into())
        .spawn(move || {
            let st = run(&app, &d, target);
            emit(&app, &st);
            d.running.store(false, Ordering::SeqCst);
        })
        .expect("dictation thread");
}

fn fail(target: &str, error: String) -> DictationState {
    DictationState {
        state: "error",
        target: target.into(),
        error: Some(error),
        ..Default::default()
    }
}

fn run(app: &AppHandle, d: &Dictation, target: &'static str) -> DictationState {
    let listening = |level: f32, seconds: f32| DictationState {
        state: "listening",
        level,
        seconds,
        target: target.into(),
        ..Default::default()
    };
    emit(app, &listening(0.0, 0.0));
    let samples = match record(d, |level, secs| emit(app, &listening(level, secs))) {
        Ok(s) => s,
        Err(e) => return fail(target, e),
    };
    emit(
        app,
        &DictationState {
            state: "transcribing",
            target: target.into(),
            ..Default::default()
        },
    );
    let Some(speech) = app.try_state::<Arc<crate::speech::SpeechService>>() else {
        return fail(target, "moteur de transcription indisponible".into());
    };
    let text = match speech.dictate(&samples) {
        Ok(t) if !t.trim().is_empty() => t,
        Ok(_) => {
            return fail(
                target,
                "Rien de compréhensible : réessayez, plus près du micro.".into(),
            )
        }
        Err(e) => return fail(target, e),
    };
    drop(samples);
    let Some(reply) = app.try_state::<Arc<crate::reply::ReplyService>>() else {
        return fail(target, "traduction indisponible".into());
    };
    let r = match reply.translate(&text, target) {
        Ok(r) => r,
        Err(e) => {
            return DictationState {
                text: Some(text),
                ..fail(target, e)
            }
        }
    };
    let copied = crate::clipboard::set_text(&r.text).is_ok();
    crate::logs::info(
        "dictation",
        &format!("réponse dictée traduite ({}, {})", target, r.via),
    );
    DictationState {
        state: "done",
        text: Some(text),
        translated: Some(r.text),
        target: target.into(),
        via: Some(r.via),
        copied,
        ..Default::default()
    }
}

/// Records the microphone until the end of the sentence; 16 kHz mono.
fn record(d: &Dictation, mut on_level: impl FnMut(f32, f32)) -> Result<Vec<f32>, String> {
    let mut stream = crate::audio::open_microphone().map_err(|e| e.to_string())?;
    let format = stream.format();
    let mut conditioner = Conditioner::new(format.sample_rate);
    let mut samples: Vec<f32> = Vec::with_capacity(SAMPLE_RATE as usize * 4);
    let mut endpoint = Endpointer::default();
    let mut analysed = 0usize;
    let mut last_level = Instant::now();
    let mut peak_db = -90.0f32;
    let mut mono = Vec::new();
    let end = loop {
        let mut packets_failed = None;
        let res = stream.read(50, &mut |p| match p {
            Packet::Audio(block) => {
                mono.clear();
                mono.extend(crate::audio::convert::mono_frames(
                    block.samples,
                    block.channels,
                ));
                conditioner.process(&mono, &mut samples);
            }
            Packet::Silence { frames, .. } => {
                let n = (frames as u64 * SAMPLE_RATE as u64 / format.sample_rate.max(1) as u64)
                    as usize;
                samples.resize(samples.len() + n, 0.0);
            }
        });
        if let Err(e) = res {
            packets_failed = Some(e.to_string());
        }
        if let Some(e) = packets_failed {
            return Err(e);
        }
        let mut stop = None;
        while analysed + FRAME <= samples.len() {
            let db = rms_db(&samples[analysed..analysed + FRAME]);
            peak_db = peak_db.max(db);
            analysed += FRAME;
            if stop.is_none() {
                stop = endpoint.frame(db);
            }
        }
        if last_level.elapsed() >= Duration::from_millis(100) {
            last_level = Instant::now();
            on_level(
                ((peak_db + 60.0) / 60.0).clamp(0.0, 1.0),
                samples.len() as f32 / SAMPLE_RATE as f32,
            );
            peak_db = -90.0;
        }
        if d.stop.load(Ordering::SeqCst) {
            break EndReason::Silence;
        }
        if let Some(r) = stop {
            break r;
        }
    };
    drop(stream); // the microphone is released before transcribing
    if !endpoint.heard_speech() || end == EndReason::NoSpeech {
        return Err("Aucune parole entendue : vérifiez le micro par défaut de Windows.".into());
    }
    Ok(samples)
}

#[tauri::command]
pub fn dictation_toggle(
    app: AppHandle,
    recent_langs: Vec<String>,
    target: Option<String>,
) -> Result<(), String> {
    let target = match target.as_deref() {
        Some("ru") => "ru",
        Some("en") => "en",
        _ => crate::reply::guess_target(&recent_langs),
    };
    let running = app
        .try_state::<Arc<Dictation>>()
        .is_some_and(|d| d.is_running());
    if !running {
        crate::reply::open(&app, target)?;
    }
    toggle(&app, target);
    Ok(())
}

#[tauri::command]
pub fn dictation_state(d: tauri::State<'_, Arc<Dictation>>) -> Option<DictationState> {
    d.last.lock().unwrap().clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(levels: &[(f32, usize)]) -> (Option<EndReason>, usize, bool) {
        let mut e = Endpointer::default();
        let mut n = 0;
        for &(db, count) in levels {
            for _ in 0..count {
                n += 1;
                if let Some(r) = e.frame(db) {
                    return (Some(r), n, e.heard_speech());
                }
            }
        }
        (None, n, e.heard_speech())
    }

    #[test]
    fn a_sentence_ends_after_a_second_of_silence() {
        // 0.4 s of room noise, 1.5 s of speech, then silence.
        let (end, frames, heard) = run(&[(-55.0, 20), (-25.0, 75), (-60.0, 200)]);
        assert_eq!(end, Some(EndReason::Silence));
        assert!(heard);
        assert_eq!(frames, 20 + 75 + 55, "1.1 s of silence after the speech");
    }

    #[test]
    fn short_pauses_do_not_cut_the_sentence() {
        let (end, frames, _) = run(&[(-25.0, 50), (-60.0, 30), (-25.0, 50), (-60.0, 200)]);
        assert_eq!(end, Some(EndReason::Silence));
        assert_eq!(frames, 50 + 30 + 50 + 55);
    }

    #[test]
    fn nothing_said_or_too_long() {
        let (end, frames, heard) = run(&[(-60.0, 1000)]);
        assert_eq!(
            (end, frames, heard),
            (Some(EndReason::NoSpeech), 250, false)
        );
        // A click is not speech.
        let (end, _, heard) = run(&[(-20.0, 2), (-60.0, 1000)]);
        assert_eq!((end, heard), (Some(EndReason::NoSpeech), false));
        let (end, frames, _) = run(&[(-25.0, 2000)]);
        assert_eq!((end, frames), (Some(EndReason::MaxLength), 600));
    }
}
