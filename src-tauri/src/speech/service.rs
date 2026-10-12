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
//! Nothing is written to disk. `purge` (stop, emergency, CS2 closed) drops the
//! open segment, the queue and any result still being computed.

use std::collections::{BTreeMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{sync_channel, Receiver, RecvTimeoutError, SyncSender, TrySendError};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use super::cpu::{lower_current_thread_priority, CpuMeter};
use super::dsp::{Conditioner, SAMPLE_RATE};
use super::engine::{default_threads, RecognizeOptions, Recognizer};
use super::filter::{judge, prompt_from};
use super::segmenter::{Outcome, Segment, Segmenter, SegmenterConfig};
use super::types::{
    EngineState, RejectReason, SpeechConfig, SpeechStats, SpeechStatus, Transcript, VoiceActivity,
};
use super::vad::{VoiceDetector, FRAME, FRAME_MS};
use super::SpeechError;
use crate::audio::{AudioBlock, BlockSink};

/// Segments older than this when Whisper gets to them are dropped: a callout
/// shown 6 s late is misleading.
pub const STALE_AFTER: Duration = Duration::from_secs(6);
/// Segments waiting for Whisper; the oldest is dropped beyond this.
pub const QUEUE_CAPACITY: usize = 3;
/// Captured blocks buffered between the capture and the VAD (~10 ms each).
const CHANNEL_BLOCKS: usize = 256;
/// The VAD runs on this many frames at a time (6 x 32 ms = 192 ms).
pub const VAD_HOP_FRAMES: usize = 6;
const STATS_EVERY: Duration = Duration::from_secs(2);
const LATENCY_WINDOW: usize = 100;

pub trait SpeechEvents: Send + Sync + 'static {
    fn status(&self, status: &SpeechStatus);
    fn transcript(&self, transcript: &Transcript);
    fn activity(&self, activity: VoiceActivity);
    fn stats(&self, stats: &SpeechStats);
}

/// Loads the models (whisper.cpp in the app, fakes in tests).
pub trait EngineLoader: Send + Sync + 'static {
    fn load_detector(&self, dir: &Path) -> Result<Box<dyn VoiceDetector>, SpeechError>;
    fn load_recognizer(&self, dir: &Path, model: &str) -> Result<Box<dyn Recognizer>, SpeechError>;
    /// Path of the model file if installed (for the status).
    fn model_path(&self, dir: &Path, model: &str) -> Option<PathBuf>;
    fn backend(&self) -> String;
}

enum FrontMsg {
    Audio {
        mono: Vec<f32>,
        rate: u32,
        at: Instant,
    },
    Threshold(f32),
    Detector(Box<dyn VoiceDetector>),
}

struct Job {
    segment: Segment,
    voice_end: Instant,
    generation: u64,
}

/// Bounded queue that drops the oldest job when full.
struct JobQueue {
    jobs: Mutex<VecDeque<Job>>,
    ready: Condvar,
}

impl JobQueue {
    fn push(&self, job: Job) -> Option<Job> {
        let mut q = self.jobs.lock().unwrap();
        let dropped = if q.len() >= QUEUE_CAPACITY {
            q.pop_front()
        } else {
            None
        };
        q.push_back(job);
        self.ready.notify_one();
        dropped
    }

    fn pop(&self, timeout: Duration) -> Option<Job> {
        let q = self.jobs.lock().unwrap();
        let (mut q, _) = self
            .ready
            .wait_timeout_while(q, timeout, |q| q.is_empty())
            .unwrap();
        q.pop_front()
    }

    fn clear(&self) {
        self.jobs.lock().unwrap().clear();
    }
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
}

struct Inner {
    events: Arc<dyn SpeechEvents>,
    loader: Arc<dyn EngineLoader>,
    models_dir: PathBuf,
    config: Mutex<SpeechConfig>,
    engine: Mutex<EngineSlot>,
    recognizer: Mutex<Option<Box<dyn Recognizer>>>,
    queue: JobQueue,
    stats: Mutex<StatsAcc>,
    dropped_blocks: AtomicU64,
    /// Bumped by `purge`: older segments and results are discarded.
    generation: AtomicU64,
    next_id: AtomicU64,
    speaking: AtomicBool,
    transcribing: AtomicBool,
    last_audio: Mutex<Option<Instant>>,
    shutdown: AtomicBool,
    threads: usize,
}

impl Inner {
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
            backend: self.loader.backend(),
            threads: self.threads as u32,
            config,
        }
    }

    fn publish_status(&self) {
        self.events.status(&self.status());
    }

    fn publish_stats(&self) {
        let s = self
            .stats
            .lock()
            .unwrap()
            .snapshot(self.dropped_blocks.load(Ordering::Relaxed));
        self.events.stats(&s);
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
        let inner = Arc::new(Inner {
            events,
            loader,
            models_dir,
            config: Mutex::new(config.sanitized()),
            engine: Mutex::new(EngineSlot {
                loading: false,
                error: None,
                load_ms: None,
                vad_ready: false,
                loaded_model: None,
            }),
            recognizer: Mutex::new(None),
            queue: JobQueue {
                jobs: Mutex::new(VecDeque::new()),
                ready: Condvar::new(),
            },
            stats: Mutex::new(StatsAcc::default()),
            dropped_blocks: AtomicU64::new(0),
            generation: AtomicU64::new(0),
            next_id: AtomicU64::new(1),
            speaking: AtomicBool::new(false),
            transcribing: AtomicBool::new(false),
            last_audio: Mutex::new(None),
            shutdown: AtomicBool::new(false),
            threads: default_threads(),
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
        self.inner
            .stats
            .lock()
            .unwrap()
            .snapshot(self.inner.dropped_blocks.load(Ordering::Relaxed))
    }

    pub fn models_dir(&self) -> &Path {
        &self.inner.models_dir
    }

    /// Applies new settings; reloads the model only if it changed.
    pub fn configure(&self, config: SpeechConfig) -> SpeechStatus {
        let config = config.sanitized();
        let model_changed = {
            let mut c = self.inner.config.lock().unwrap();
            let changed = c.model != config.model;
            *c = config.clone();
            changed
        };
        let _ = self
            .front
            .try_send(FrontMsg::Threshold(config.vad_threshold));
        if model_changed {
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
                let model = inner.config.lock().unwrap().model.clone();
                let mut error = None;
                if with_vad {
                    match inner.loader.load_detector(&inner.models_dir) {
                        Ok(d) => {
                            // Blocking send: the detector must reach the VAD thread.
                            let _ = front.send(FrontMsg::Detector(d));
                            inner.engine.lock().unwrap().vad_ready = true;
                        }
                        Err(e) => error = Some(e),
                    }
                }
                let loaded = match inner.loader.load_recognizer(&inner.models_dir, &model) {
                    Ok(r) => {
                        *inner.recognizer.lock().unwrap() = Some(r);
                        true
                    }
                    Err(e) => {
                        error.get_or_insert(e);
                        false
                    }
                };
                {
                    let mut e = inner.engine.lock().unwrap();
                    e.loading = false;
                    if loaded {
                        e.loaded_model = Some(model);
                        e.load_ms = Some(started.elapsed().as_millis() as u32);
                    }
                    // A missing file is a normal state (shown as such), not an error.
                    e.error = match error {
                        Some(SpeechError::NotInstalled(_)) | None => None,
                        Some(err) => Some(err.to_string()),
                    };
                }
                inner.publish_status();
            })
            .expect("spawn speech-load thread");
    }

    /// Drops everything in flight: open segment, queued segments, running result.
    pub fn purge(&self) {
        self.inner.generation.fetch_add(1, Ordering::SeqCst);
        self.inner.queue.clear();
        *self.inner.last_audio.lock().unwrap() = None;
        self.inner.set_activity(Some(false), Some(false));
    }

    /// The consumer to plug into the audio capture.
    pub fn sink(&self) -> SpeechSink {
        SpeechSink {
            tx: self.front.clone(),
            inner: self.inner.clone(),
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
}

impl SpeechSink {
    fn send(&self, mono: Vec<f32>, rate: u32) {
        let msg = FrontMsg::Audio {
            mono,
            rate,
            at: Instant::now(),
        };
        if let Err(TrySendError::Full(_)) = self.tx.try_send(msg) {
            self.inner.dropped_blocks.fetch_add(1, Ordering::Relaxed);
        }
    }
}

impl BlockSink for SpeechSink {
    fn block(&mut self, block: &AudioBlock) {
        let mono: Vec<f32> =
            crate::audio::convert::mono_frames(block.samples, block.channels).collect();
        self.send(mono, block.sample_rate);
    }

    fn silence(&mut self, frames: usize, _channels: u16, sample_rate: u32) {
        // Silence must flow too: it is what closes a segment.
        self.send(vec![0.0; frames], sample_rate);
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
}

impl Front {
    fn new(threshold: f32, generation: u64) -> Self {
        Self {
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
        match msg {
            FrontMsg::Threshold(t) => st.segmenter.set_threshold(t),
            FrontMsg::Detector(d) => {
                detector = Some(d);
                st = Front::new(st.segmenter.config().threshold, generation);
            }
            FrontMsg::Audio { mono, rate, at } => {
                *inner.last_audio.lock().unwrap() = Some(at);
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
                eprintln!("crosstalk: {e}");
                vec![0.0; VAD_HOP_FRAMES]
            }
        };
        for (frame, prob) in chunk.chunks_exact(FRAME).zip(probs) {
            let outcome = st.segmenter.push(frame, prob);
            inner.set_activity(Some(st.segmenter.is_speaking()), None);
            match outcome {
                None => {}
                Some(Outcome::Rejected(r)) => {
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
                    let dropped = inner.queue.push(Job {
                        segment,
                        voice_end,
                        generation: st.generation,
                    });
                    if dropped.is_some() {
                        inner.stats.lock().unwrap().reject(RejectReason::Overflow);
                        inner.publish_stats();
                    }
                }
            }
        }
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
        if job.voice_end.elapsed() > STALE_AFTER {
            inner.stats.lock().unwrap().reject(RejectReason::Stale);
            inner.publish_stats();
            continue;
        }
        transcribe(inner, job);
    }
}

fn transcribe(inner: &Inner, job: Job) {
    let config = inner.config.lock().unwrap().clone();
    let prompt = prompt_from(&config.glossary);
    let mut slot = inner.recognizer.lock().unwrap();
    let Some(recognizer) = slot.as_mut() else {
        return; // model not installed or loading
    };
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
            eprintln!("crosstalk: {e}");
            return;
        }
    };
    match judge(&rec, audio_ms) {
        Err(reason) => {
            stats.reject(reason);
            drop(stats);
        }
        Ok(text) => {
            let latency_ms = job.voice_end.elapsed().as_millis() as u32;
            stats.transcribed += 1;
            stats.latencies.push_back(latency_ms);
            if stats.latencies.len() > LATENCY_WINDOW {
                stats.latencies.pop_front();
            }
            drop(stats);
            inner.events.transcript(&Transcript {
                id: inner.next_id.fetch_add(1, Ordering::SeqCst),
                text,
                lang: rec.lang,
                lang_prob: rec.lang_prob,
                audio_ms,
                latency_ms,
                asr_ms,
                avg_logprob: rec.avg_logprob,
                no_speech_prob: rec.no_speech_prob,
                model: config.model.clone(),
            });
        }
    }
    inner.publish_stats();
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
    }

    /// Returns a fixed text after an optional delay; counts calls.
    struct EchoRecognizer {
        text: String,
        delay: Duration,
        calls: Arc<AtomicU64>,
    }
    impl Recognizer for EchoRecognizer {
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
            jobs: Mutex::new(VecDeque::new()),
            ready: Condvar::new(),
        };
        let job = |n: u64| Job {
            segment: Segment {
                samples: vec![],
                start_frame: n,
                last_voiced_frame: n,
                voiced_frames: 0,
                forced: false,
            },
            voice_end: Instant::now(),
            generation: 0,
        };
        for n in 0..QUEUE_CAPACITY as u64 {
            assert!(q.push(job(n)).is_none());
        }
        assert_eq!(q.push(job(9)).unwrap().segment.start_frame, 0);
        assert_eq!(q.pop(Duration::ZERO).unwrap().segment.start_frame, 1);
        q.clear();
        assert!(q.pop(Duration::from_millis(1)).is_none());
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
}
