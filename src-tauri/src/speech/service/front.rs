//! The front of the pipeline: the capture thread hands the audio over
//! (`SpeechSink`, downmix only), and the VAD thread conditions it, detects
//! voices, segments them and queues them for Whisper.

use std::sync::atomic::Ordering;
use std::sync::mpsc::{Receiver, RecvTimeoutError, SyncSender, TrySendError};
use std::sync::Arc;
use std::time::{Duration, Instant};

use super::fallback::Lag;
use super::{
    diagnostic_rejection, FrontMsg, Inner, Job, PARTIAL_MAX_RTF, PARTIAL_MIN_FRAMES,
    PARTIAL_STEP_FRAMES, VAD_HOP_FRAMES,
};
use crate::audio::{AudioBlock, BlockSink};
use crate::speech::dsp::{Conditioner, SAMPLE_RATE};
use crate::speech::focus::VoiceFocus;
use crate::speech::segmenter::{Outcome, Segmenter, SegmenterConfig};
use crate::speech::types::RejectReason;
use crate::speech::vad::{VoiceDetector, FRAME, FRAME_MS};

/// Runs on the capture thread: downmix and hand over, nothing else.
pub struct SpeechSink {
    pub(super) tx: SyncSender<FrontMsg>,
    pub(super) inner: Arc<Inner>,
    /// "Focus voix" state while it is on and the source is stereo.
    pub(super) focus: Option<VoiceFocus>,
    pub(super) focus_frames: usize,
}

impl SpeechSink {
    fn send(&self, mono: Vec<f32>, rate: u32, generation: u64) {
        let msg = FrontMsg::Audio {
            mono,
            rate,
            at: Instant::now(),
            generation,
        };
        if let Err(TrySendError::Full(_)) = self.tx.try_send(msg) {
            self.inner.dropped_blocks.fetch_add(1, Ordering::Relaxed);
        }
    }
}

impl BlockSink for SpeechSink {
    fn block(&mut self, block: &AudioBlock) {
        let generation = self.inner.generation.load(Ordering::SeqCst);
        let mono: Vec<f32> = if block.channels == 2 && self.inner.focus.load(Ordering::Relaxed) {
            let f = self.focus.get_or_insert_with(Default::default);
            let mut out = Vec::with_capacity(block.samples.len() / 2);
            f.process(block.samples, 2, &mut out);
            // Meter: what was removed over about a second.
            self.focus_frames += block.samples.len() / 2;
            if self.focus_frames >= block.sample_rate as usize {
                self.focus_frames = 0;
                *self.inner.focus_db.lock().unwrap() = f.take_reduction_db();
            }
            out
        } else {
            if self.focus.take().is_some() {
                *self.inner.focus_db.lock().unwrap() = None;
            }
            crate::audio::convert::mono_frames(block.samples, block.channels).collect()
        };
        self.send(mono, block.sample_rate, generation);
    }

    fn silence(&mut self, frames: usize, _channels: u16, sample_rate: u32) {
        // Silence must flow too: it is what closes a segment.
        let generation = self.inner.generation.load(Ordering::SeqCst);
        self.send(vec![0.0; frames], sample_rate, generation);
    }
}

/// Maps 16 kHz frame indices to wall-clock instants, re-anchored after gaps.
pub(super) struct FrameClock {
    origin: Option<Instant>,
    samples: u64,
}

impl FrameClock {
    pub(super) fn new() -> Self {
        Self {
            origin: None,
            samples: 0,
        }
    }

    pub(super) fn advance(&mut self, produced: usize, at: Instant) {
        self.samples += produced as u64;
        let elapsed = Duration::from_secs_f64(self.samples as f64 / SAMPLE_RATE as f64);
        match self.origin {
            None => self.origin = at.checked_sub(elapsed).or(Some(at)),
            Some(o) => {
                let expected = o + elapsed;
                if at > expected + Duration::from_millis(300) {
                    self.origin = Some(o + (at - expected));
                }
            }
        }
    }

    pub(super) fn instant_of(&self, frame: u64) -> Instant {
        let o = self.origin.unwrap_or_else(Instant::now);
        o + Duration::from_millis(frame * FRAME_MS as u64)
    }
}

struct Front {
    conditioner: Option<Conditioner>,
    segmenter: Segmenter,
    clock: FrameClock,
    pending: Vec<f32>,
    generation: u64,
    /// Id of the utterance being recorded.
    utterance: Option<u64>,
    /// Open-segment length at the last partial request.
    partial_frames: usize,
}

impl Front {
    fn new(threshold: f32, generation: u64) -> Self {
        Self {
            utterance: None,
            partial_frames: 0,
            conditioner: None,
            segmenter: Segmenter::new(SegmenterConfig::new(threshold)),
            clock: FrameClock::new(),
            pending: Vec::new(),
            generation,
        }
    }
}

pub(super) fn front_loop(inner: &Inner, rx: Receiver<FrontMsg>) {
    let threshold = inner.config.lock().unwrap().vad_threshold;
    let mut detector: Option<Box<dyn VoiceDetector>> = None;
    let mut st = Front::new(threshold, inner.generation.load(Ordering::SeqCst));

    loop {
        let msg = match rx.recv_timeout(Duration::from_millis(250)) {
            Ok(m) => m,
            Err(RecvTimeoutError::Timeout) => {
                if inner.shutdown.load(Ordering::SeqCst) {
                    return;
                }
                continue;
            }
            Err(RecvTimeoutError::Disconnected) => return,
        };
        let generation = inner.generation.load(Ordering::SeqCst);
        if generation != st.generation {
            let threshold = st.segmenter.config().threshold;
            st = Front::new(threshold, generation);
            if let Some(d) = detector.as_mut() {
                d.reset();
            }
        }
        st.segmenter
            .set_threshold(inner.config.lock().unwrap().vad_threshold);
        match msg {
            FrontMsg::Detector(d) => {
                detector = Some(d);
                st = Front::new(st.segmenter.config().threshold, generation);
            }
            FrontMsg::Audio {
                mono,
                rate,
                at,
                generation: captured_generation,
            } => {
                {
                    let _publication = inner.publication.lock().unwrap();
                    if captured_generation != inner.generation.load(Ordering::SeqCst) {
                        continue;
                    }
                    *inner.last_audio.lock().unwrap() = Some(at);
                }
                let Some(det) = detector.as_mut() else {
                    continue; // no VAD model: nothing can be segmented
                };
                if st.conditioner.as_ref().map(|c| c.input_rate()) != Some(rate) {
                    let threshold = st.segmenter.config().threshold;
                    st = Front::new(threshold, generation);
                    st.conditioner = Some(Conditioner::new(rate));
                    det.reset();
                }
                let before = st.pending.len();
                if let Some(c) = st.conditioner.as_mut() {
                    c.process(&mono, &mut st.pending);
                }
                st.clock.advance(st.pending.len() - before, at);
                run_vad(inner, det.as_mut(), &mut st);
            }
        }
    }
}

fn run_vad(inner: &Inner, det: &mut dyn VoiceDetector, st: &mut Front) {
    let hop = VAD_HOP_FRAMES * FRAME;
    while st.pending.len() >= hop {
        let chunk: Vec<f32> = st.pending.drain(..hop).collect();
        let probs = match det.probabilities(&chunk) {
            Ok(p) => p,
            Err(e) => {
                crate::logs::warn("vad", &e.to_string());
                vec![0.0; VAD_HOP_FRAMES]
            }
        };
        let _publication = inner.publication.lock().unwrap();
        if st.generation != inner.generation.load(Ordering::SeqCst) {
            return;
        }
        for (frame, prob) in chunk.chunks_exact(FRAME).zip(probs) {
            let outcome = st.segmenter.push(frame, prob);
            inner.set_activity(Some(st.segmenter.is_speaking()), None);
            match outcome {
                None => {}
                Some(Outcome::Rejected(r)) => {
                    if let Some(id) = st.utterance.take() {
                        inner.close_utterance(id);
                    }
                    st.partial_frames = 0;
                    diagnostic_rejection(r, 0, 0);
                    {
                        let mut s = inner.stats.lock().unwrap();
                        s.segments += 1;
                        s.reject(r);
                    }
                    inner.publish_stats();
                }
                Some(Outcome::Segment(segment)) => {
                    let voice_end = st.clock.instant_of(segment.last_voiced_frame + 1);
                    inner.stats.lock().unwrap().segments += 1;
                    let utterance = st
                        .utterance
                        .take()
                        .unwrap_or_else(|| inner.next_id.fetch_add(1, Ordering::SeqCst));
                    st.partial_frames = 0;
                    let dropped = inner.queue.push(Job {
                        segment,
                        voice_end,
                        queued_at: Instant::now(),
                        generation: st.generation,
                        utterance,
                        partial: false,
                    });
                    if let Some(dropped) = dropped {
                        inner.close_utterance(dropped.utterance);
                        diagnostic_rejection(
                            RejectReason::Overflow,
                            dropped.segment.duration_ms(),
                            dropped.queued_at.elapsed().as_millis(),
                        );
                        inner.stats.lock().unwrap().reject(RejectReason::Overflow);
                        inner.publish_stats();
                        inner.note_lag(Lag::Overflow);
                    }
                }
            }
            if st.segmenter.is_speaking() && st.utterance.is_none() {
                st.utterance = Some(inner.next_id.fetch_add(1, Ordering::SeqCst));
                st.partial_frames = 0;
            }
        }
        request_partial(inner, st);
    }
}

/// Live words while someone speaks: the open segment goes to Whisper when it
/// is idle, at most once per `PARTIAL_STEP_FRAMES` of new audio.
fn request_partial(inner: &Inner, st: &mut Front) {
    let (Some(utterance), n) = (st.utterance, st.segmenter.open_frames()) else {
        return;
    };
    // Paced by Whisper's speed: at least twice its last live time between two
    // live lines, and none when it cannot keep up (CPU taken by the game).
    let partial_ms = inner.live.lock().unwrap().partial_ms as usize;
    let step = PARTIAL_STEP_FRAMES.max(2 * partial_ms / FRAME_MS as usize);
    if n < PARTIAL_MIN_FRAMES || n < st.partial_frames + step {
        return;
    }
    if partial_ms as f32 > PARTIAL_MAX_RTF * (n * FRAME_MS as usize) as f32 {
        return;
    }
    if inner.transcribing.load(Ordering::SeqCst) {
        return; // retried at the next hop
    }
    let Some(segment) = st.segmenter.open_segment() else {
        return;
    };
    let now = Instant::now();
    let job = Job {
        segment,
        voice_end: now,
        queued_at: now,
        generation: st.generation,
        utterance,
        partial: true,
    };
    if inner.queue.push_partial(job) {
        st.partial_frames = n;
    }
}
