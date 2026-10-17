//! The recognition thread: final segments first, then live lines; stale checks,
//! inference, hallucination filter, then the transcript event.

use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::{Duration, Instant};

use super::{diagnostic_rejection, Inner, Job, LIVE_FALLBACK_AFTER, STALE_AFTER};
use crate::speech::engine::RecognizeOptions;
use crate::speech::filter::{judge, Recognition};
use crate::speech::types::{RejectReason, Transcript};

/// Statistics are published this often while audio flows.
const STATS_EVERY: Duration = Duration::from_secs(2);

/// The loaded model, if it is the one configured (not a superseded one still in place).
fn loaded_model(inner: &Inner, config: &crate::speech::SpeechConfig) -> Option<String> {
    let loaded = inner.engine.lock().unwrap().loaded_model.clone()?;
    (loaded == config.model).then_some(loaded)
}

pub(super) fn asr_loop(inner: &Arc<Inner>) {
    let mut last_stats = Instant::now();
    while !inner.shutdown.load(Ordering::SeqCst) {
        let job = inner.queue.pop(Duration::from_millis(500));
        if last_stats.elapsed() >= STATS_EVERY {
            last_stats = Instant::now();
            let active = inner
                .last_audio
                .lock()
                .unwrap()
                .is_some_and(|t| t.elapsed() < Duration::from_secs(3));
            if active {
                let mut s = inner.stats.lock().unwrap();
                s.cpu_percent = s.cpu.sample();
                drop(s);
                inner.publish_stats();
            }
        }
        let Some(job) = job else { continue };
        if job.generation != inner.generation.load(Ordering::SeqCst) {
            continue;
        }
        if job.partial {
            transcribe_partial(inner, job);
            continue;
        }
        // Too late for another recognition pass: the live text becomes final.
        if job.voice_end.elapsed() > LIVE_FALLBACK_AFTER && finalize_from_live(inner, &job) {
            continue;
        }
        if job.voice_end.elapsed() > STALE_AFTER && !inner.partial_shown(job.utterance) {
            inner.close_utterance(job.utterance);
            diagnostic_rejection(
                RejectReason::Stale,
                job.segment.duration_ms(),
                job.queued_at.elapsed().as_millis(),
            );
            inner.stats.lock().unwrap().reject(RejectReason::Stale);
            inner.publish_stats();
            continue;
        }
        transcribe(inner, job);
    }
}

/// Emits the last live transcript of the job's utterance as its final line
/// (no new recognition pass); false when no live line is on screen.
pub(super) fn finalize_from_live(inner: &Inner, job: &Job) -> bool {
    let _publication = inner.publication.lock().unwrap();
    if job.generation != inner.generation.load(Ordering::SeqCst) {
        return true; // purged: nothing to emit
    }
    let last = {
        let mut live = inner.live.lock().unwrap();
        let Some(t) = live.last.remove(&job.utterance) else {
            return false;
        };
        live.shown.remove(&job.utterance);
        t
    };
    let latency_ms = job.voice_end.elapsed().as_millis() as u32;
    let close_ms = close_ms(job);
    let queue_ms = job.queued_at.elapsed().as_millis() as u32;
    inner
        .stats
        .lock()
        .unwrap()
        .transcribed(latency_ms, [close_ms, queue_ms, 0]);
    inner.events.transcript(&Transcript {
        partial: false,
        latency_ms,
        audio_ms: job.segment.duration_ms(),
        close_ms,
        queue_ms,
        asr_ms: 0,
        emitted_at_ms: now_ms(),
        ..last
    });
    inner.publish_stats();
    true
}

/// End of the voice → segment closed and queued.
fn close_ms(job: &Job) -> u32 {
    job.queued_at
        .saturating_duration_since(job.voice_end)
        .as_millis() as u32
}

/// Unix time in ms, for the interface's own delay.
pub(super) fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

/// Judge again after inference: a fresh queue entry may expire during computation.
pub(super) fn judge_current(
    rec: &Recognition,
    audio_ms: u32,
    age: Duration,
) -> Result<String, RejectReason> {
    if age > STALE_AFTER {
        Err(RejectReason::Stale)
    } else {
        judge(rec, audio_ms)
    }
}

pub(super) fn transcribe(inner: &Inner, job: Job) {
    let config = inner.config.lock().unwrap().clone();
    let mut slot = inner.recognizer.lock().unwrap();
    let Some(model) = loaded_model(inner, &config) else {
        inner.close_utterance(job.utterance);
        return;
    };
    let Some(recognizer) = slot.as_mut() else {
        inner.close_utterance(job.utterance);
        return; // model not installed or loading
    };
    let publication = inner.publication.lock().unwrap();
    if job.generation != inner.generation.load(Ordering::SeqCst) {
        return;
    }
    let shown = inner.partial_shown(job.utterance);
    if job.voice_end.elapsed() > STALE_AFTER && !shown {
        inner.close_utterance(job.utterance);
        diagnostic_rejection(
            RejectReason::Stale,
            job.segment.duration_ms(),
            job.queued_at.elapsed().as_millis(),
        );
        inner.stats.lock().unwrap().reject(RejectReason::Stale);
        inner.publish_stats();
        drop(publication);
        drop(slot);
        return;
    }
    let queue_ms = job.queued_at.elapsed().as_millis();
    inner.set_activity(None, Some(true));
    drop(publication);
    let started = Instant::now();
    let result = recognizer.recognize(
        &job.segment.samples,
        &RecognizeOptions {
            languages: &config.languages,
        },
    );
    drop(slot);
    let asr_ms = started.elapsed().as_millis() as u32;
    inner.set_activity(None, Some(false));
    let _publication = inner.publication.lock().unwrap();
    if job.generation != inner.generation.load(Ordering::SeqCst) {
        return; // purged while transcribing
    }
    let audio_ms = job.segment.duration_ms();
    let rtf = asr_ms as f32 / audio_ms.max(1) as f32;
    let mut stats = inner.stats.lock().unwrap();
    stats.last_rtf = Some(rtf);
    stats.asr_ms += asr_ms as u64;
    stats.asr_audio_ms += audio_ms as u64;
    let rec = match result {
        Ok(r) => r,
        Err(e) => {
            drop(stats);
            if std::env::var_os("CROSSTALK_SPEECH_DIAGNOSTICS").is_some_and(|v| v == "1") {
                eprintln!("crosstalk speech diagnostic: inference_error={e:?} audio_ms={audio_ms} queue_ms={queue_ms} inference_ms={asr_ms}");
            }
            crate::logs::warn("asr", &format!("transcription impossible : {e}"));
            inner.close_utterance(job.utterance);
            return;
        }
    };
    // A line already shown live is completed even if late.
    let age = if shown {
        Duration::ZERO
    } else {
        job.voice_end.elapsed()
    };
    let outcome = judge_current(&rec, audio_ms, age);
    if std::env::var_os("CROSSTALK_SPEECH_DIAGNOSTICS").is_some_and(|v| v == "1") {
        eprintln!("crosstalk speech diagnostic: model={} audio_ms={} queue_ms={} inference_ms={} lang={:?} lang_prob={} outcome={:?} raw={:?}",
            model, audio_ms, queue_ms, asr_ms, rec.lang, rec.lang_prob, outcome, rec.text);
    }
    match outcome {
        Err(reason) => {
            stats.reject(reason);
            drop(stats);
            inner.close_utterance(job.utterance);
        }
        Ok(text) => {
            let latency_ms = job.voice_end.elapsed().as_millis() as u32;
            let close_ms = close_ms(&job);
            stats.transcribed(latency_ms, [close_ms, queue_ms as u32, asr_ms]);
            drop(stats);
            {
                let mut live = inner.live.lock().unwrap();
                live.shown.remove(&job.utterance);
                live.last.remove(&job.utterance);
            }
            inner.events.transcript(&Transcript {
                id: job.utterance,
                partial: false,
                text,
                lang: rec.lang,
                lang_prob: rec.lang_prob,
                audio_ms,
                latency_ms,
                asr_ms,
                close_ms,
                queue_ms: queue_ms as u32,
                emitted_at_ms: now_ms(),
                model,
                speaker: None,
            });
        }
    }
    inner.publish_stats();
}

/// Live words of the segment being recorded: shown, never counted in the
/// statistics, replaced by the final transcript of the same id.
pub(super) fn transcribe_partial(inner: &Inner, job: Job) {
    let config = inner.config.lock().unwrap().clone();
    let mut slot = inner.recognizer.lock().unwrap();
    let Some(model) = loaded_model(inner, &config) else {
        return;
    };
    let Some(recognizer) = slot.as_mut() else {
        return;
    };
    if job.generation != inner.generation.load(Ordering::SeqCst) {
        return;
    }
    inner.set_activity(None, Some(true));
    let queue_ms = job.queued_at.elapsed().as_millis() as u32;
    let started = Instant::now();
    let result = recognizer.recognize(
        &job.segment.samples,
        &RecognizeOptions {
            languages: &config.languages,
        },
    );
    drop(slot);
    let asr_ms = started.elapsed().as_millis() as u32;
    inner.set_activity(None, Some(false));
    inner.live.lock().unwrap().partial_ms = asr_ms;
    let audio_ms = job.segment.duration_ms();
    let Ok(rec) = result else { return };
    let Ok(text) = judge(&rec, audio_ms) else {
        return; // nothing worth showing yet
    };
    let _publication = inner.publication.lock().unwrap();
    if job.generation != inner.generation.load(Ordering::SeqCst) {
        return;
    }
    {
        let mut live = inner.live.lock().unwrap();
        if live.closed.contains(&job.utterance) {
            return;
        }
        live.shown.insert(job.utterance);
    }
    let t = Transcript {
        id: job.utterance,
        partial: true,
        text,
        lang: rec.lang,
        lang_prob: rec.lang_prob,
        audio_ms,
        latency_ms: asr_ms,
        asr_ms,
        close_ms: 0,
        queue_ms,
        emitted_at_ms: now_ms(),
        model,
        speaker: None,
    };
    inner
        .live
        .lock()
        .unwrap()
        .last
        .insert(job.utterance, t.clone());
    inner.events.transcript(&t);
}
