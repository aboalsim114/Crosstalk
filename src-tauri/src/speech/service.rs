//! Speech service: receives the captured audio, detects voices, segments them
//! and transcribes them, off the UI thread and off the async runtime.
//!
//! ```text
//! capture thread ──BlockSink──▶ [bounded channel] ──▶ speech-vad thread
//!   (downmix only)                drop when full        16 kHz, high-pass, Silero, segmenter
//!                                                              │
//!                                       [queue of 3, drops the oldest segment]
//!                                                              ▼
//!                                         speech-asr thread (below normal priority)
//!                                         stale check, Whisper, filter ──▶ events
//! ```
//!
//! While someone speaks, the open segment is also transcribed every second
//! (from 1.5 s on) when Whisper is idle: these **partial** transcripts let the
//! UI show the words live. The final transcript of the segment, with the same
//! id, replaces them; if the segment is finally rejected, `retract` removes
//! the line. Final segments always go first.
//!
//! Nothing is written to disk. `purge` (stop, emergency, CS2 closed) drops the
//! open segment, the queue and any result still being computed.

use std::collections::{BTreeMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{sync_channel, Receiver, RecvTimeoutError, SyncSender, TrySendError};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use super::dsp::{Conditioner, SAMPLE_RATE};
use super::engine::{default_threads, sanitize_threads, RecognizeOptions, Recognizer};
use super::filter::{judge, prompt_with};
use super::segmenter::{Outcome, Segment, Segmenter, SegmenterConfig};
use super::types::{
    EngineState, RejectReason, SpeechConfig, SpeechStats, SpeechStatus, Transcript, VoiceActivity,
};
use super::vad::{VoiceDetector, FRAME, FRAME_MS};
use super::SpeechError;
use crate::audio::{AudioBlock, BlockSink};
use crate::cpu::{lower_current_thread_priority, CpuMeter};

/// Segments older than this when Whisper gets to them are dropped: a callout
/// shown 6 s late is misleading.
pub const STALE_AFTER: Duration = Duration::from_secs(6);
/// Segments waiting for Whisper; the oldest is dropped beyond this.
pub const QUEUE_CAPACITY: usize = 3;
/// Captured blocks buffered between the capture and the VAD (~10 ms each).
const CHANNEL_BLOCKS: usize = 256;
/// The VAD runs on this many frames at a time (6 x 32 ms = 192 ms).
pub const VAD_HOP_FRAMES: usize = 6;
/// A partial transcript needs this much open segment (47 x 32 ms = 1.5 s)...
pub const PARTIAL_MIN_FRAMES: usize = 47;
/// ...and this much new audio since the previous one (31 x 32 ms = 1 s).
pub const PARTIAL_STEP_FRAMES: usize = 31;
/// Live lines stop while Whisper needs more than this share of the audio
/// duration: the final lines must not wait behind them.
pub const PARTIAL_MAX_RTF: f32 = 0.5;
/// A final segment this late after the voice ended, whose live line is on
/// screen, becomes final with that live text instead of waiting for Whisper.
pub const LIVE_FALLBACK_AFTER: Duration = Duration::from_secs(3);
const STATS_EVERY: Duration = Duration::from_secs(2);
const LATENCY_WINDOW: usize = 100;

pub trait SpeechEvents: Send + Sync + 'static {
    fn status(&self, status: &SpeechStatus);
    fn transcript(&self, transcript: &Transcript);
    fn activity(&self, activity: VoiceActivity);
    fn stats(&self, stats: &SpeechStats);
    /// The line of a partial transcript must go: its segment was rejected.
    fn retract(&self, id: u64);
}

/// Loads the models (whisper.cpp in the app, fakes in tests).
pub trait EngineLoader: Send + Sync + 'static {
    fn load_detector(&self, dir: &Path) -> Result<Box<dyn VoiceDetector>, SpeechError>;
    fn load_recognizer(
        &self,
        dir: &Path,
        model: &str,
        accel: super::types::Accel,
    ) -> Result<Box<dyn Recognizer>, SpeechError>;
    /// Path of the model file if installed (for the status).
    fn model_path(&self, dir: &Path, model: &str) -> Option<PathBuf>;
    fn backend(&self) -> String;
}

enum FrontMsg {
    Audio {
        mono: Vec<f32>,
        rate: u32,
        at: Instant,
        generation: u64,
    },
    Detector(Box<dyn VoiceDetector>),
}

struct Job {
    segment: Segment,
    voice_end: Instant,
    queued_at: Instant,
    generation: u64,
    /// Id of the utterance: shared by its partial and final transcripts.
    utterance: u64,
    partial: bool,
}

#[derive(Default)]
struct Jobs {
    finals: VecDeque<Job>,
    /// The latest partial request (an older one is useless).
    partial: Option<Job>,
}

/// Bounded queue of final segments (drops the oldest when full), plus one
/// partial slot served only when no final segment waits.
struct JobQueue {
    jobs: Mutex<Jobs>,
    ready: Condvar,
}

impl JobQueue {
    fn push(&self, job: Job) -> Option<Job> {
        let mut q = self.jobs.lock().unwrap();
        if q.partial
            .as_ref()
            .is_some_and(|p| p.utterance == job.utterance)
        {
            q.partial = None;
        }
        let dropped = if q.finals.len() >= QUEUE_CAPACITY {
            q.finals.pop_front()
        } else {
            None
        };
        q.finals.push_back(job);
        self.ready.notify_one();
        dropped
    }

    /// Asks for a partial transcript; false when final segments are waiting.
    fn push_partial(&self, job: Job) -> bool {
        let mut q = self.jobs.lock().unwrap();
        if !q.finals.is_empty() {
            return false;
        }
        q.partial = Some(job);
        self.ready.notify_one();
        true
    }

    fn pop(&self, timeout: Duration) -> Option<Job> {
        let q = self.jobs.lock().unwrap();
        let (mut q, _) = self
            .ready
            .wait_timeout_while(q, timeout, |q| q.finals.is_empty() && q.partial.is_none())
            .unwrap();
        q.finals.pop_front().or_else(|| q.partial.take())
    }

    fn clear(&self) {
        let mut q = self.jobs.lock().unwrap();
        q.finals.clear();
        q.partial = None;
    }
}

/// Utterances whose partial transcript is on screen, and utterances closed
/// without a final transcript (a late partial result must not reappear).
#[derive(Default)]
struct Live {
    shown: std::collections::HashSet<u64>,
    closed: VecDeque<u64>,
    /// Last live transcript of each shown utterance: the final line when the
    /// segment comes too late for another Whisper pass.
    last: std::collections::HashMap<u64, Transcript>,
    /// Whisper time of the last live transcript, to pace the next ones.
    partial_ms: u32,
}

#[derive(Default)]
struct StatsAcc {
    segments: u64,
    transcribed: u64,
    rejected: BTreeMap<RejectReason, u64>,
    latencies: VecDeque<u32>,
    last_rtf: Option<f32>,
    cpu: CpuMeter,
    cpu_percent: Option<f32>,
}

impl StatsAcc {
    fn reject(&mut self, r: RejectReason) {
        *self.rejected.entry(r).or_default() += 1;
    }

    fn snapshot(&self, dropped_blocks: u64) -> SpeechStats {
        let mut sorted: Vec<u32> = self.latencies.iter().copied().collect();
        sorted.sort_unstable();
        let pct = |q: f32| {
            (!sorted.is_empty())
                .then(|| sorted[((q * sorted.len() as f32) as usize).min(sorted.len() - 1)])
        };
        SpeechStats {
            segments: self.segments,
            transcribed: self.transcribed,
            rejected: self.rejected.clone(),
            dropped_blocks,
            latency_p50_ms: pct(0.5),
            latency_p95_ms: pct(0.95),
            last_rtf: self.last_rtf,
            cpu_percent: self.cpu_percent,
            focus_db: None,
        }
    }
}

struct EngineSlot {
    loading: bool,
    error: Option<String>,
    load_ms: Option<u32>,
    vad_ready: bool,
    /// Model id of the loaded recogniser.
    loaded_model: Option<String>,
    /// Where it runs (CPU or GPU name), once loaded.
    loaded_backend: Option<String>,
}

struct Inner {
    events: Arc<dyn SpeechEvents>,
    loader: Arc<dyn EngineLoader>,
    models_dir: PathBuf,
    config: Mutex<SpeechConfig>,
    /// Callouts of the map being played (Game State Integration).
    map_prompt: Mutex<Vec<String>>,
    engine: Mutex<EngineSlot>,
    recognizer: Mutex<Option<Box<dyn Recognizer>>>,
    queue: JobQueue,
    live: Mutex<Live>,
    stats: Mutex<StatsAcc>,
    dropped_blocks: AtomicU64,
    /// Bumped by `purge`: older segments and results are discarded.
    generation: AtomicU64,
    publication: Mutex<()>,
    next_id: AtomicU64,
    speaking: AtomicBool,
    transcribing: AtomicBool,
    last_audio: Mutex<Option<Instant>>,
    shutdown: AtomicBool,
    threads: usize,
    /// "Focus voix" on (read by the capture thread).
    focus: AtomicBool,
    focus_db: Mutex<Option<f32>>,
}

impl Inner {
    fn stats_snapshot(&self) -> SpeechStats {
        let mut s = self
            .stats
            .lock()
            .unwrap()
            .snapshot(self.dropped_blocks.load(Ordering::Relaxed));
        s.focus_db = *self.focus_db.lock().unwrap();
        s
    }

    fn status(&self) -> SpeechStatus {
        let config = self.config.lock().unwrap().clone();
        let e = self.engine.lock().unwrap();
        let model_path = self.loader.model_path(&self.models_dir, &config.model);
        let recognizer_ready = e.loaded_model.as_deref() == Some(config.model.as_str());
        let state = if e.loading {
            EngineState::Loading
        } else if e.error.is_some() {
            EngineState::Error
        } else if recognizer_ready && e.vad_ready {
            EngineState::Ready
        } else {
            EngineState::Missing
        };
        SpeechStatus {
            state,
            model_path: model_path.map(|p| p.display().to_string()),
            vad_ready: e.vad_ready,
            models_dir: self.models_dir.display().to_string(),
            error: e.error.clone(),
            load_ms: e.load_ms,
            backend: e
                .loaded_backend
                .clone()
                .unwrap_or_else(|| self.loader.backend()),
            threads: self.threads as u32,
            config,
        }
    }

    fn publish_status(&self) {
        self.events.status(&self.status());
    }

    fn publish_stats(&self) {
        let s = self.stats_snapshot();
        self.events.stats(&s);
    }

    /// Removes the partial line of an utterance closed without a transcript.
    fn close_utterance(&self, id: u64) {
        let mut live = self.live.lock().unwrap();
        live.closed.push_back(id);
        if live.closed.len() > 64 {
            live.closed.pop_front();
        }
        live.last.remove(&id);
        if live.shown.remove(&id) {
            drop(live);
            self.events.retract(id);
        }
    }

    fn partial_shown(&self, id: u64) -> bool {
        self.live.lock().unwrap().shown.contains(&id)
    }

    fn set_activity(&self, speaking: Option<bool>, transcribing: Option<bool>) {
        let mut changed = false;
        if let Some(v) = speaking {
            changed |= self.speaking.swap(v, Ordering::SeqCst) != v;
        }
        if let Some(v) = transcribing {
            changed |= self.transcribing.swap(v, Ordering::SeqCst) != v;
        }
        if changed {
            self.events.activity(VoiceActivity {
                speaking: self.speaking.load(Ordering::SeqCst),
                transcribing: self.transcribing.load(Ordering::SeqCst),
            });
        }
    }
}

pub struct SpeechService {
    inner: Arc<Inner>,
    front: SyncSender<FrontMsg>,
}

impl SpeechService {
    /// Starts the worker threads and loads the models in the background.
    pub fn start(
        events: Arc<dyn SpeechEvents>,
        loader: Arc<dyn EngineLoader>,
        models_dir: PathBuf,
        config: SpeechConfig,
    ) -> Arc<Self> {
        Self::start_with_threads(events, loader, models_dir, config, default_threads())
    }

    /// Explicit worker count for reproducible live benchmarks.
    pub fn start_with_threads(
        events: Arc<dyn SpeechEvents>,
        loader: Arc<dyn EngineLoader>,
        models_dir: PathBuf,
        config: SpeechConfig,
        threads: usize,
    ) -> Arc<Self> {
        let focus = config.voice_focus;
        let inner = Arc::new(Inner {
            focus: AtomicBool::new(focus),
            focus_db: Mutex::new(None),
            events,
            loader,
            models_dir,
            config: Mutex::new(config.sanitized()),
            map_prompt: Mutex::new(Vec::new()),
            engine: Mutex::new(EngineSlot {
                loading: false,
                error: None,
                load_ms: None,
                vad_ready: false,
                loaded_model: None,
                loaded_backend: None,
            }),
            recognizer: Mutex::new(None),
            live: Mutex::new(Live::default()),
            queue: JobQueue {
                jobs: Mutex::new(Jobs::default()),
                ready: Condvar::new(),
            },
            stats: Mutex::new(StatsAcc::default()),
            dropped_blocks: AtomicU64::new(0),
            generation: AtomicU64::new(0),
            publication: Mutex::new(()),
            next_id: AtomicU64::new(1),
            speaking: AtomicBool::new(false),
            transcribing: AtomicBool::new(false),
            last_audio: Mutex::new(None),
            shutdown: AtomicBool::new(false),
            threads: sanitize_threads(threads),
        });
        let (tx, rx) = sync_channel(CHANNEL_BLOCKS);
        let svc = Arc::new(Self { inner, front: tx });

        let inner = svc.inner.clone();
        std::thread::Builder::new()
            .name("speech-vad".into())
            .spawn(move || front_loop(&inner, rx))
            .expect("spawn speech-vad thread");
        let inner = svc.inner.clone();
        std::thread::Builder::new()
            .name("speech-asr".into())
            .spawn(move || {
                lower_current_thread_priority();
                asr_loop(&inner)
            })
            .expect("spawn speech-asr thread");

        svc.reload(true);
        svc
    }

    pub fn status(&self) -> SpeechStatus {
        self.inner.status()
    }

    pub fn stats(&self) -> SpeechStats {
        self.inner.stats_snapshot()
    }

    pub fn models_dir(&self) -> &Path {
        &self.inner.models_dir
    }

    /// The user's dictated reply (16 kHz mono) in their language, with the
    /// model that transcribes the game: waits for a callout in progress, then runs.
    pub fn dictate(&self, samples: &[f32], lang: &str) -> Result<String, String> {
        let prompt = {
            let config = self.inner.config.lock().unwrap();
            prompt_with(&config.glossary, &self.inner.map_prompt.lock().unwrap())
        };
        let mut slot = self.inner.recognizer.lock().unwrap();
        let recognizer = slot
            .as_mut()
            .ok_or("modèle Whisper pas encore chargé (Audio & IA)")?;
        recognizer
            .transcribe_in(samples, lang, &prompt, self.inner.threads)
            .map_err(|e| e.to_string())
    }

    /// Callouts of the map being played, added to Whisper's prompt (A-06).
    pub fn set_map_prompt(&self, callouts: Vec<String>) {
        *self.inner.map_prompt.lock().unwrap() = callouts;
    }

    /// Applies new settings; reloads the model only if it changed.
    pub fn configure(&self, config: SpeechConfig) -> SpeechStatus {
        let config = config.sanitized();
        let model_changed = {
            let mut c = self.inner.config.lock().unwrap();
            let changed = c.model != config.model || c.accel != config.accel;
            *c = config.clone();
            self.inner.focus.store(config.voice_focus, Ordering::SeqCst);
            changed
        };
        if model_changed {
            self.purge();
            self.reload(false);
        }
        self.inner.publish_status();
        self.status()
    }

    /// Looks for the models again (after installing them) and loads what is missing.
    pub fn rescan(&self) -> SpeechStatus {
        let need_vad = !self.inner.engine.lock().unwrap().vad_ready;
        self.reload(need_vad);
        self.status()
    }

    fn reload(&self, with_vad: bool) {
        let inner = self.inner.clone();
        let front = self.front.clone();
        {
            let mut e = inner.engine.lock().unwrap();
            if e.loading {
                return;
            }
            e.loading = true;
            e.error = None;
        }
        inner.publish_status();
        std::thread::Builder::new()
            .name("speech-load".into())
            .spawn(move || {
                let started = Instant::now();
                let mut detector_error = None;
                if with_vad {
                    match inner.loader.load_detector(&inner.models_dir) {
                        Ok(d) => {
                            if front.send(FrontMsg::Detector(d)).is_ok() {
                                inner.engine.lock().unwrap().vad_ready = true;
                            }
                        }
                        Err(e) => detector_error = Some(e),
                    }
                }
                loop {
                    let (model, accel) = {
                        let c = inner.config.lock().unwrap();
                        (c.model.clone(), c.accel)
                    };
                    let result = inner
                        .loader
                        .load_recognizer(&inner.models_dir, &model, accel);
                    // Configuration may change during a slow load. Never install a
                    // superseded model, and keep loading until the current one wins.
                    // Wait for in-flight inference without holding the settings
                    // lock: status/configure must remain responsive.
                    let mut recognizer = inner.recognizer.lock().unwrap();
                    let config = inner.config.lock().unwrap();
                    if config.model != model || config.accel != accel {
                        continue;
                    }
                    let mut error = detector_error.take();
                    match result {
                        Ok(r) => {
                            let backend = r.backend();
                            *recognizer = Some(r);
                            let mut e = inner.engine.lock().unwrap();
                            e.loaded_model = Some(model);
                            e.loaded_backend = backend;
                            e.load_ms = Some(started.elapsed().as_millis() as u32);
                        }
                        Err(err) => {
                            *recognizer = None;
                            inner.engine.lock().unwrap().loaded_model = None;
                            error.get_or_insert(err);
                        }
                    }
                    let mut e = inner.engine.lock().unwrap();
                    e.loading = false;
                    e.error = match error {
                        Some(SpeechError::NotInstalled(_)) | None => None,
                        Some(err) => Some(err.to_string()),
                    };
                    break;
                }
                inner.publish_status();
            })
            .expect("spawn speech-load thread");
    }

    /// Drops everything in flight: open segment, queued segments, running result.
    pub fn purge(&self) {
        let _publication = self.inner.publication.lock().unwrap();
        self.inner.generation.fetch_add(1, Ordering::SeqCst);
        self.inner.queue.clear();
        let shown: Vec<u64> = {
            let mut live = self.inner.live.lock().unwrap();
            live.last.clear();
            live.shown.drain().collect()
        };
        for id in shown {
            self.inner.events.retract(id);
        }
        *self.inner.last_audio.lock().unwrap() = None;
        self.inner.set_activity(Some(false), Some(false));
    }

    /// The consumer to plug into the audio capture.
    pub fn sink(&self) -> SpeechSink {
        SpeechSink {
            tx: self.front.clone(),
            inner: self.inner.clone(),
            focus: None,
            focus_frames: 0,
        }
    }
}

impl Drop for SpeechService {
    fn drop(&mut self) {
        self.inner.shutdown.store(true, Ordering::SeqCst);
    }
}

/// Runs on the capture thread: downmix and hand over, nothing else.
pub struct SpeechSink {
    tx: SyncSender<FrontMsg>,
    inner: Arc<Inner>,
    /// "Focus voix" state while it is on and the source is stereo.
    focus: Option<super::focus::VoiceFocus>,
    focus_frames: usize,
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
struct FrameClock {
    origin: Option<Instant>,
    samples: u64,
}

impl FrameClock {
    fn new() -> Self {
        Self {
            origin: None,
            samples: 0,
        }
    }

    fn advance(&mut self, produced: usize, at: Instant) {
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

    fn instant_of(&self, frame: u64) -> Instant {
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

fn front_loop(inner: &Inner, rx: Receiver<FrontMsg>) {
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

fn diagnostic_rejection(reason: RejectReason, audio_ms: u32, queue_ms: u128) {
    if std::env::var_os("CROSSTALK_SPEECH_DIAGNOSTICS").is_some_and(|v| v == "1") {
        eprintln!("crosstalk speech diagnostic: outcome={reason:?} audio_ms={audio_ms} queue_ms={queue_ms} inference=not_run raw=None");
    }
}

fn asr_loop(inner: &Inner) {
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
        // Too late for another Whisper pass: the live text becomes final.
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
/// (no new Whisper pass); false when no live line is on screen.
fn finalize_from_live(inner: &Inner, job: &Job) -> bool {
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
    {
        let mut stats = inner.stats.lock().unwrap();
        stats.transcribed += 1;
        stats.latencies.push_back(latency_ms);
        if stats.latencies.len() > LATENCY_WINDOW {
            stats.latencies.pop_front();
        }
    }
    inner.events.transcript(&Transcript {
        partial: false,
        latency_ms,
        audio_ms: job.segment.duration_ms(),
        ..last
    });
    inner.publish_stats();
    true
}

/// Judge again after inference: a fresh queue entry may expire during computation.
fn judge_current(
    rec: &super::filter::Recognition,
    audio_ms: u32,
    age: Duration,
) -> Result<String, RejectReason> {
    if age > STALE_AFTER {
        Err(RejectReason::Stale)
    } else {
        judge(rec, audio_ms)
    }
}

fn transcribe(inner: &Inner, job: Job) {
    let config = inner.config.lock().unwrap().clone();
    let prompt = prompt_with(&config.glossary, &inner.map_prompt.lock().unwrap());
    let mut slot = inner.recognizer.lock().unwrap();
    if inner.engine.lock().unwrap().loaded_model.as_deref() != Some(config.model.as_str()) {
        inner.close_utterance(job.utterance);
        return;
    }
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
            prompt: &prompt,
            threads: inner.threads,
            fast_encoder: config.fast_encoder,
        },
    );
    let timing = recognizer.timing();
    drop(slot);
    let asr_ms = started.elapsed().as_millis() as u32;
    inner.set_activity(None, Some(false));
    let _publication = inner.publication.lock().unwrap();
    if job.generation != inner.generation.load(Ordering::SeqCst) {
        return; // purged while transcribing
    }
    let audio_ms = job.segment.duration_ms();
    let mut stats = inner.stats.lock().unwrap();
    stats.last_rtf = Some(asr_ms as f32 / audio_ms.max(1) as f32);
    let rec = match result {
        Ok(r) => r,
        Err(e) => {
            drop(stats);
            if std::env::var_os("CROSSTALK_SPEECH_DIAGNOSTICS").is_some_and(|v| v == "1") {
                eprintln!("crosstalk speech diagnostic: inference_error={e:?} audio_ms={audio_ms} queue_ms={queue_ms} inference_ms={asr_ms} timing={timing:?}");
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
        eprintln!("crosstalk speech diagnostic: model={} encoder={} audio_ms={} queue_ms={} inference_ms={} timing={:?} lang={:?} lang_prob={} logprob={} no_speech={} outcome={:?} raw={:?}",
            config.model, if config.fast_encoder { "reduced" } else { "full" }, audio_ms, queue_ms,
            asr_ms, timing, rec.lang, rec.lang_prob, rec.avg_logprob, rec.no_speech_prob, outcome, rec.text);
    }
    match outcome {
        Err(reason) => {
            stats.reject(reason);
            drop(stats);
            inner.close_utterance(job.utterance);
        }
        Ok(text) => {
            let latency_ms = job.voice_end.elapsed().as_millis() as u32;
            stats.transcribed += 1;
            stats.latencies.push_back(latency_ms);
            if stats.latencies.len() > LATENCY_WINDOW {
                stats.latencies.pop_front();
            }
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
                avg_logprob: rec.avg_logprob,
                no_speech_prob: rec.no_speech_prob,
                model: config.model.clone(),
                speaker: None,
            });
        }
    }
    inner.publish_stats();
}

/// Live words of the segment being recorded: shown, never counted in the
/// statistics, replaced by the final transcript of the same id.
fn transcribe_partial(inner: &Inner, job: Job) {
    let config = inner.config.lock().unwrap().clone();
    let prompt = prompt_with(&config.glossary, &inner.map_prompt.lock().unwrap());
    let mut slot = inner.recognizer.lock().unwrap();
    if inner.engine.lock().unwrap().loaded_model.as_deref() != Some(config.model.as_str()) {
        return;
    }
    let Some(recognizer) = slot.as_mut() else {
        return;
    };
    if job.generation != inner.generation.load(Ordering::SeqCst) {
        return;
    }
    inner.set_activity(None, Some(true));
    let started = Instant::now();
    let result = recognizer.recognize(
        &job.segment.samples,
        &RecognizeOptions {
            languages: &config.languages,
            prompt: &prompt,
            threads: inner.threads,
            fast_encoder: config.fast_encoder,
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
        avg_logprob: rec.avg_logprob,
        no_speech_prob: rec.no_speech_prob,
        model: config.model.clone(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::speech::filter::Recognition;
    use crate::speech::types::Lang;
    use crate::speech::vad::EnergyDetector;
    use std::sync::mpsc::{channel, Receiver, Sender};

    #[derive(Debug)]
    #[allow(dead_code)] // payloads are kept for failure messages
    enum Ev {
        Status(SpeechStatus),
        Transcript(Transcript),
        Activity(VoiceActivity),
        Stats(SpeechStats),
        Retract(u64),
    }

    struct Events(Mutex<Sender<Ev>>);
    impl SpeechEvents for Events {
        fn status(&self, s: &SpeechStatus) {
            let _ = self.0.lock().unwrap().send(Ev::Status(s.clone()));
        }
        fn transcript(&self, t: &Transcript) {
            let _ = self.0.lock().unwrap().send(Ev::Transcript(t.clone()));
        }
        fn activity(&self, a: VoiceActivity) {
            let _ = self.0.lock().unwrap().send(Ev::Activity(a));
        }
        fn stats(&self, s: &SpeechStats) {
            let _ = self.0.lock().unwrap().send(Ev::Stats(s.clone()));
        }
        fn retract(&self, id: u64) {
            let _ = self.0.lock().unwrap().send(Ev::Retract(id));
        }
    }

    /// Returns a fixed text after an optional delay; counts calls.
    struct EchoRecognizer {
        text: String,
        delay: Duration,
        calls: Arc<AtomicU64>,
    }
    impl Recognizer for EchoRecognizer {
        fn transcribe_in(
            &mut self,
            samples: &[f32],
            lang: &str,
            prompt: &str,
            _threads: usize,
        ) -> Result<String, SpeechError> {
            assert!(prompt.contains("rush B"), "CS words help the dictation too");
            Ok(format!("{lang}:{}", samples.len()))
        }
        fn recognize(
            &mut self,
            _samples: &[f32],
            opts: &RecognizeOptions,
        ) -> Result<Recognition, SpeechError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            std::thread::sleep(self.delay);
            assert!(opts.prompt.contains("rush B"));
            assert!(opts.fast_encoder);
            Ok(Recognition {
                text: self.text.clone(),
                lang: opts.languages[0],
                lang_prob: 1.0,
                avg_logprob: -0.2,
                no_speech_prob: 0.01,
            })
        }
    }

    struct FakeLoader {
        text: String,
        delay: Duration,
        calls: Arc<AtomicU64>,
        has_model: bool,
    }
    impl EngineLoader for FakeLoader {
        fn load_detector(&self, _dir: &Path) -> Result<Box<dyn VoiceDetector>, SpeechError> {
            Ok(Box::new(EnergyDetector {
                threshold_db: -40.0,
            }))
        }
        fn load_recognizer(
            &self,
            _dir: &Path,
            model: &str,
            _accel: crate::speech::types::Accel,
        ) -> Result<Box<dyn Recognizer>, SpeechError> {
            if !self.has_model {
                return Err(SpeechError::NotInstalled(model.into()));
            }
            Ok(Box::new(EchoRecognizer {
                text: self.text.clone(),
                delay: self.delay,
                calls: self.calls.clone(),
            }))
        }
        fn model_path(&self, dir: &Path, model: &str) -> Option<PathBuf> {
            self.has_model.then(|| dir.join(model))
        }
        fn backend(&self) -> String {
            "test".into()
        }
    }

    fn service(
        text: &str,
        delay_ms: u64,
        has_model: bool,
    ) -> (Arc<SpeechService>, Receiver<Ev>, Arc<AtomicU64>) {
        let (tx, rx) = channel();
        let calls = Arc::new(AtomicU64::new(0));
        let svc = SpeechService::start(
            Arc::new(Events(Mutex::new(tx))),
            Arc::new(FakeLoader {
                text: text.into(),
                delay: Duration::from_millis(delay_ms),
                calls: calls.clone(),
                has_model,
            }),
            PathBuf::from("models"),
            SpeechConfig {
                languages: vec![Lang::Ru],
                ..SpeechConfig::default()
            },
        );
        (svc, rx, calls)
    }

    #[test]
    fn dictation_uses_the_loaded_model_in_french() {
        let (svc, _rx, _) = service("unused", 0, false);
        let end = Instant::now() + Duration::from_secs(5);
        while svc.status().state == EngineState::Loading {
            assert!(Instant::now() < end);
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(svc
            .dictate(&[0.0; 16], "fr")
            .unwrap_err()
            .contains("pas encore chargé"));
        let (svc, _rx, _) = service("unused", 0, true);
        wait_ready(&svc);
        assert_eq!(svc.dictate(&[0.0; 16_000], "fr").unwrap(), "fr:16000");
    }

    fn wait_ready(svc: &SpeechService) {
        let end = Instant::now() + Duration::from_secs(5);
        while svc.status().state != EngineState::Ready {
            assert!(Instant::now() < end, "engine not ready: {:?}", svc.status());
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    /// 48 kHz stereo: `voice_ms` of tone then `silence_ms` of silence, in 10 ms blocks.
    fn feed(sink: &mut SpeechSink, voice_ms: usize, silence_ms: usize) {
        let block = 480;
        for i in 0..(voice_ms / 10) {
            let samples: Vec<f32> = (0..block * 2)
                .map(|j| 0.3 * ((i * block + j / 2) as f32 * 0.06).sin())
                .collect();
            sink.block(&AudioBlock {
                samples: &samples,
                channels: 2,
                sample_rate: 48_000,
                qpc_100ns: 0,
            });
        }
        for _ in 0..(silence_ms / 10) {
            sink.silence(block, 2, 48_000);
        }
    }

    fn next_transcript(rx: &Receiver<Ev>) -> Option<Transcript> {
        let end = Instant::now() + Duration::from_secs(5);
        while let Some(left) = end.checked_duration_since(Instant::now()) {
            match rx.recv_timeout(left) {
                Ok(Ev::Transcript(t)) => return Some(t),
                Ok(_) => continue,
                Err(_) => return None,
            }
        }
        None
    }

    #[test]
    fn transcribes_an_utterance_end_to_end() {
        let (svc, rx, _) = service("Двое на B", 0, true);
        wait_ready(&svc);
        let mut sink = svc.sink();
        feed(&mut sink, 800, 700);
        let t = next_transcript(&rx).expect("transcript");
        assert_eq!(t.text, "Двое на B");
        assert_eq!(t.lang, Lang::Ru);
        assert!(t.audio_ms >= 800, "{}", t.audio_ms);
        assert!(t.latency_ms < 3000);
        let st = svc.stats();
        assert_eq!(st.transcribed, 1);
        assert!(st.latency_p50_ms.is_some());
    }

    #[test]
    fn voice_focus_keeps_the_centred_voice_and_reports_what_it_removes() {
        let (svc, rx, _) = service("Двое на B", 0, true);
        wait_ready(&svc);
        assert_eq!(svc.stats().focus_db, None, "off by default");
        svc.configure(SpeechConfig {
            voice_focus: true,
            ..svc.status().config
        });
        let mut sink = svc.sink();
        // The test tone is the same on both channels: a centred voice.
        feed(&mut sink, 1200, 700);
        let t = next_transcript(&rx).expect("the centred voice still goes through");
        assert_eq!(t.text, "Двое на B");
        let removed = svc.stats().focus_db.expect("meter");
        assert!(
            removed < 1.0,
            "a centred voice is not removed ({removed} dB)"
        );
        svc.configure(SpeechConfig {
            voice_focus: false,
            ..svc.status().config
        });
        feed(&mut sink, 20, 0);
        assert_eq!(svc.stats().focus_db, None);
    }

    #[test]
    fn rejects_hallucinations_without_emitting() {
        let (svc, rx, calls) = service("Продолжение следует...", 0, true);
        wait_ready(&svc);
        let mut sink = svc.sink();
        feed(&mut sink, 800, 700);
        assert!(next_transcript(&rx).is_none());
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert_eq!(
            svc.stats().rejected.get(&RejectReason::Hallucination),
            Some(&1)
        );
    }

    #[test]
    fn purge_discards_the_result_being_computed() {
        let (svc, rx, calls) = service("Бомба на A", 400, true);
        wait_ready(&svc);
        let mut sink = svc.sink();
        feed(&mut sink, 800, 700);
        let end = Instant::now() + Duration::from_secs(5);
        while calls.load(Ordering::SeqCst) == 0 {
            assert!(Instant::now() < end);
            std::thread::sleep(Duration::from_millis(5));
        }
        svc.purge();
        assert!(next_transcript(&rx).is_none());
        assert_eq!(svc.stats().transcribed, 0);
    }

    #[test]
    fn without_model_nothing_is_transcribed() {
        let (svc, rx, calls) = service("x", 0, false);
        let end = Instant::now() + Duration::from_secs(5);
        while svc.status().state == EngineState::Loading {
            assert!(Instant::now() < end);
            std::thread::sleep(Duration::from_millis(5));
        }
        let st = svc.status();
        assert_eq!(st.state, EngineState::Missing);
        assert!(st.vad_ready);
        assert_eq!(st.error, None, "a missing model is not an error");
        let mut sink = svc.sink();
        feed(&mut sink, 800, 700);
        assert!(next_transcript(&rx).is_none());
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn queue_drops_the_oldest_segment() {
        let q = JobQueue {
            jobs: Mutex::new(Jobs::default()),
            ready: Condvar::new(),
        };
        let job = |n: u64| Job {
            utterance: n,
            partial: false,
            segment: Segment {
                samples: vec![],
                start_frame: n,
                last_voiced_frame: n,
                voiced_frames: 0,
                forced: false,
            },
            voice_end: Instant::now(),
            queued_at: Instant::now(),
            generation: 0,
        };
        for n in 0..QUEUE_CAPACITY as u64 {
            assert!(q.push(job(n)).is_none());
        }
        assert_eq!(q.push(job(9)).unwrap().segment.start_frame, 0);
        assert_eq!(q.pop(Duration::ZERO).unwrap().segment.start_frame, 1);
        // Partials wait for the final segments, and the latest one wins.
        assert!(!q.push_partial(Job {
            partial: true,
            ..job(20)
        }));
        q.clear();
        assert!(q.pop(Duration::from_millis(1)).is_none());
        assert!(q.push_partial(Job {
            partial: true,
            ..job(20)
        }));
        assert!(q.push_partial(Job {
            partial: true,
            ..job(21)
        }));
        let p = q.pop(Duration::ZERO).unwrap();
        assert!(p.partial && p.utterance == 21);
        // The final segment of an utterance cancels its pending partial.
        assert!(q.push_partial(Job {
            partial: true,
            ..job(22)
        }));
        q.push(job(22));
        assert!(!q.pop(Duration::ZERO).unwrap().partial);
        assert!(q.pop(Duration::from_millis(1)).is_none());
    }

    /// Answers each call with the next text of the list (the last one repeats).
    struct Script(Vec<&'static str>, usize);
    impl Recognizer for Script {
        fn recognize(
            &mut self,
            _: &[f32],
            opts: &RecognizeOptions,
        ) -> Result<Recognition, SpeechError> {
            let text = self.0[self.1.min(self.0.len() - 1)];
            self.1 += 1;
            Ok(Recognition {
                text: text.into(),
                lang: opts.languages[0],
                lang_prob: 1.0,
                avg_logprob: -0.2,
                no_speech_prob: 0.01,
            })
        }
    }

    fn next_event(rx: &Receiver<Ev>) -> Option<Ev> {
        let end = Instant::now() + Duration::from_secs(5);
        while let Some(left) = end.checked_duration_since(Instant::now()) {
            match rx.recv_timeout(left) {
                Ok(e @ (Ev::Transcript(_) | Ev::Retract(_))) => return Some(e),
                Ok(_) => continue,
                Err(_) => return None,
            }
        }
        None
    }

    #[test]
    fn long_utterances_show_live_words_then_the_final_line() {
        let (svc, rx, _) = service("unused", 0, true);
        wait_ready(&svc);
        *svc.inner.recognizer.lock().unwrap() =
            Some(Box::new(Script(vec!["Двое", "Двое идут на B"], 0)));
        let mut sink = svc.sink();
        feed(&mut sink, 2000, 0);
        let Some(Ev::Transcript(p)) = next_event(&rx) else {
            panic!("no partial")
        };
        assert!(p.partial);
        assert_eq!(p.text, "Двое");
        feed(&mut sink, 300, 700);
        // Possibly more partials (one per second of speech), then the final line.
        let f = loop {
            match next_event(&rx) {
                Some(Ev::Transcript(t)) if t.partial => assert_eq!(t.id, p.id),
                Some(Ev::Transcript(t)) => break t,
                e => panic!("expected the final transcript, got {e:?}"),
            }
        };
        assert_eq!((f.id, f.text.as_str()), (p.id, "Двое идут на B"));
        // Partials are not counted as transcripts.
        assert_eq!(svc.stats().transcribed, 1);
    }

    #[test]
    fn a_rejected_final_retracts_the_live_line() {
        let (svc, rx, _) = service("unused", 0, true);
        wait_ready(&svc);
        *svc.inner.recognizer.lock().unwrap() =
            Some(Box::new(Script(vec!["Бомба", "Продолжение следует..."], 0)));
        let mut sink = svc.sink();
        feed(&mut sink, 2000, 0);
        let Some(Ev::Transcript(p)) = next_event(&rx) else {
            panic!("no partial")
        };
        assert!(p.partial);
        feed(&mut sink, 300, 700);
        loop {
            match next_event(&rx) {
                Some(Ev::Transcript(t)) if t.partial => assert_eq!(t.id, p.id),
                Some(Ev::Retract(id)) => break assert_eq!(id, p.id),
                e => panic!("expected a retraction, got {e:?}"),
            }
        }
        assert!(next_event(&rx).is_none());
    }

    #[test]
    fn a_late_final_segment_takes_its_live_text() {
        let (svc, rx, calls) = service("unused", 0, true);
        wait_ready(&svc);
        let live = Transcript {
            id: 42,
            partial: true,
            text: "Двое идут".into(),
            lang: Lang::Ru,
            lang_prob: 1.0,
            audio_ms: 2000,
            latency_ms: 300,
            asr_ms: 300,
            avg_logprob: -0.2,
            no_speech_prob: 0.01,
            model: "base".into(),
            speaker: None,
        };
        {
            let mut l = svc.inner.live.lock().unwrap();
            l.shown.insert(42);
            l.last.insert(42, live);
        }
        let job = Job {
            segment: Segment {
                samples: vec![0.0; 16_000 * 3],
                start_frame: 0,
                last_voiced_frame: 90,
                voiced_frames: 90,
                forced: false,
            },
            voice_end: Instant::now() - Duration::from_secs(4),
            queued_at: Instant::now(),
            generation: 0,
            utterance: 42,
            partial: false,
        };
        assert!(finalize_from_live(&svc.inner, &job));
        let t = next_transcript(&rx).unwrap();
        assert_eq!((t.id, t.partial, t.text.as_str()), (42, false, "Двое идут"));
        assert!(t.latency_ms >= 4000);
        assert_eq!(calls.load(Ordering::SeqCst), 0, "no Whisper pass");
        assert_eq!(svc.stats().transcribed, 1);
        // Without a live line, Whisper is needed.
        assert!(!finalize_from_live(
            &svc.inner,
            &Job {
                utterance: 43,
                ..job
            }
        ));
    }

    #[test]
    fn live_lines_stop_when_whisper_cannot_keep_up() {
        let (svc, rx, calls) = service("Двое на B", 0, true);
        wait_ready(&svc);
        // The last live line took 3 s of Whisper for 2 s of audio.
        svc.inner.live.lock().unwrap().partial_ms = 3000;
        let mut sink = svc.sink();
        // Two halves: the capture → VAD buffer holds 256 blocks of 10 ms.
        feed(&mut sink, 2000, 0);
        std::thread::sleep(Duration::from_millis(300));
        feed(&mut sink, 0, 700);
        let t = next_transcript(&rx).unwrap();
        assert!(!t.partial, "a live line was requested");
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn purge_retracts_live_lines() {
        let (svc, rx, _) = service("Бомба на A", 0, true);
        wait_ready(&svc);
        let mut sink = svc.sink();
        feed(&mut sink, 2000, 0);
        let Some(Ev::Transcript(p)) = next_event(&rx) else {
            panic!("no partial")
        };
        svc.purge();
        match next_event(&rx) {
            Some(Ev::Retract(id)) => assert_eq!(id, p.id),
            e => panic!("expected a retraction, got {e:?}"),
        }
    }

    #[test]
    fn frame_clock_tracks_wall_time_and_gaps() {
        let t0 = Instant::now() + Duration::from_secs(10);
        let mut c = FrameClock::new();
        c.advance(16_000, t0);
        assert_eq!(c.instant_of(0), t0 - Duration::from_secs(1));
        assert_eq!(
            c.instant_of(31),
            c.instant_of(0) + Duration::from_millis(992)
        );
        // 2 s of audio lost: re-anchored on the arrival time.
        c.advance(1_600, t0 + Duration::from_secs(3));
        assert_eq!(c.instant_of(0), t0 + Duration::from_millis(1900));
    }

    #[test]
    fn configure_keeps_the_model_when_unchanged() {
        let (svc, _rx, _) = service("x", 0, true);
        wait_ready(&svc);
        let st = svc.configure(SpeechConfig {
            vad_threshold: 0.7,
            languages: vec![Lang::En],
            ..svc.status().config
        });
        assert_eq!(st.state, EngineState::Ready);
        assert_eq!(st.config.languages, vec![Lang::En]);
        assert_eq!(st.config.vad_threshold, 0.7);
    }
    #[test]
    fn result_expiring_during_inference_is_rejected() {
        let rec = Recognition {
            text: "Двое на B".into(),
            lang: Lang::Ru,
            lang_prob: 1.0,
            avg_logprob: -0.2,
            no_speech_prob: 0.01,
        };
        assert!(judge_current(&rec, 800, STALE_AFTER).is_ok());
        assert_eq!(
            judge_current(&rec, 800, STALE_AFTER + Duration::from_nanos(1)),
            Err(RejectReason::Stale)
        );
    }

    #[test]
    fn explicit_live_threads_are_preserved() {
        struct CheckThreads;
        impl Recognizer for CheckThreads {
            fn recognize(
                &mut self,
                _: &[f32],
                opts: &RecognizeOptions,
            ) -> Result<Recognition, SpeechError> {
                assert_eq!(opts.threads, sanitize_threads(2));
                Ok(Recognition {
                    text: "Двое на B".into(),
                    lang: Lang::Ru,
                    lang_prob: 1.0,
                    avg_logprob: -0.2,
                    no_speech_prob: 0.01,
                })
            }
        }
        let (tx, rx) = channel();
        let svc = SpeechService::start_with_threads(
            Arc::new(Events(Mutex::new(tx))),
            Arc::new(FakeLoader {
                text: "x".into(),
                delay: Duration::ZERO,
                calls: Arc::new(AtomicU64::new(0)),
                has_model: true,
            }),
            PathBuf::from("models"),
            SpeechConfig::default(),
            2,
        );
        wait_ready(&svc);
        assert_eq!(svc.status().threads, sanitize_threads(2) as u32);
        *svc.inner.recognizer.lock().unwrap() = Some(Box::new(CheckThreads));
        transcribe(
            &svc.inner,
            Job {
                segment: Segment {
                    samples: vec![0.1; 12_800],
                    start_frame: 0,
                    last_voiced_frame: 24,
                    voiced_frames: 25,
                    forced: false,
                },
                voice_end: Instant::now(),
                queued_at: Instant::now(),
                generation: 0,
                utterance: 0,
                partial: false,
            },
        );
        assert_eq!(next_transcript(&rx).unwrap().text, "Двое на B");
    }

    #[test]
    fn reload_follows_configuration_changed_during_load() {
        struct GatedLoader {
            entered: Mutex<Sender<()>>,
            release: Mutex<Receiver<()>>,
            models: Mutex<Vec<String>>,
        }
        impl EngineLoader for GatedLoader {
            fn load_detector(&self, _: &Path) -> Result<Box<dyn VoiceDetector>, SpeechError> {
                Ok(Box::new(EnergyDetector {
                    threshold_db: -40.0,
                }))
            }
            fn load_recognizer(
                &self,
                _: &Path,
                model: &str,
                _: crate::speech::types::Accel,
            ) -> Result<Box<dyn Recognizer>, SpeechError> {
                let first = {
                    let mut models = self.models.lock().unwrap();
                    models.push(model.to_string());
                    models.len() == 1
                };
                if first {
                    self.entered.lock().unwrap().send(()).unwrap();
                    self.release.lock().unwrap().recv().unwrap();
                }
                Ok(Box::new(EchoRecognizer {
                    text: "x".into(),
                    delay: Duration::ZERO,
                    calls: Arc::new(AtomicU64::new(0)),
                }))
            }
            fn model_path(&self, _: &Path, _: &str) -> Option<PathBuf> {
                None
            }
            fn backend(&self) -> String {
                "test".into()
            }
        }
        let (entered_tx, entered_rx) = channel();
        let (release_tx, release_rx) = channel();
        let loader = Arc::new(GatedLoader {
            entered: Mutex::new(entered_tx),
            release: Mutex::new(release_rx),
            models: Mutex::new(Vec::new()),
        });
        let (tx, _) = channel();
        let svc = SpeechService::start(
            Arc::new(Events(Mutex::new(tx))),
            loader.clone(),
            PathBuf::from("models"),
            SpeechConfig::default(),
        );
        entered_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("first load entered");
        let initial = svc.status().config.model;
        let next = if initial == "tiny" { "base" } else { "tiny" };
        svc.configure(SpeechConfig {
            model: next.into(),
            ..svc.status().config
        });
        release_tx.send(()).unwrap();
        wait_ready(&svc);
        assert_eq!(
            svc.inner.engine.lock().unwrap().loaded_model.as_deref(),
            Some(next)
        );
        assert_eq!(*loader.models.lock().unwrap(), vec![initial, next.into()]);
    }

    #[test]
    fn purged_capture_blocks_cannot_be_relabelled_as_new_audio() {
        let (svc, _, calls) = service("Двое на B", 0, true);
        wait_ready(&svc);
        // Consume a complete pre-purge utterance synchronously after purge.
        let old = svc.inner.generation.load(Ordering::SeqCst);
        let (tx, rx) = sync_channel(256);
        tx.send(FrontMsg::Detector(Box::new(EnergyDetector {
            threshold_db: -40.0,
        })))
        .unwrap();
        for i in 0..100 {
            let mono = (0..480)
                .map(|j| 0.3 * ((i * 480 + j) as f32 * 0.06).sin())
                .collect();
            tx.send(FrontMsg::Audio {
                mono,
                rate: 48_000,
                at: Instant::now(),
                generation: old,
            })
            .unwrap();
        }
        for _ in 0..70 {
            tx.send(FrontMsg::Audio {
                mono: vec![0.0; 480],
                rate: 48_000,
                at: Instant::now(),
                generation: old,
            })
            .unwrap();
        }
        svc.purge();
        drop(tx);
        front_loop(&svc.inner, rx);
        assert_eq!(svc.stats().segments, 0);
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        assert!(svc.inner.last_audio.lock().unwrap().is_none());
    }
    #[test]
    fn purge_during_vad_cannot_restore_old_speaking_activity() {
        struct GatedDetector {
            entered: Sender<()>,
            release: Receiver<()>,
            first: bool,
        }
        impl VoiceDetector for GatedDetector {
            fn probabilities(&mut self, samples: &[f32]) -> Result<Vec<f32>, SpeechError> {
                if self.first {
                    self.first = false;
                    self.entered.send(()).unwrap();
                    self.release.recv().unwrap();
                }
                Ok(vec![1.0; samples.len() / FRAME])
            }
            fn reset(&mut self) {}
        }
        let (svc, _, _) = service("Двое на B", 0, true);
        wait_ready(&svc);
        let (entered_tx, entered_rx) = channel();
        let (release_tx, release_rx) = channel();
        let (tx, rx) = sync_channel(32);
        tx.send(FrontMsg::Detector(Box::new(GatedDetector {
            entered: entered_tx,
            release: release_rx,
            first: true,
        })))
        .unwrap();
        for _ in 0..20 {
            tx.send(FrontMsg::Audio {
                mono: vec![0.3; 480],
                rate: 48_000,
                at: Instant::now(),
                generation: 0,
            })
            .unwrap();
        }
        drop(tx);
        let inner = svc.inner.clone();
        let worker = std::thread::spawn(move || front_loop(&inner, rx));
        entered_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("VAD entered");
        svc.purge();
        release_tx.send(()).unwrap();
        worker.join().unwrap();
        assert!(!svc.inner.speaking.load(Ordering::SeqCst));
        assert_eq!(svc.stats().segments, 0);
    }
}
