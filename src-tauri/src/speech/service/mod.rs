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
//! When a heavy model cannot keep up during a capture (CS2 taking the
//! processor), the service switches to whisper-base until the capture stops
//! (`fallback`).
//!
//! Nothing is written to disk. `purge` (stop, emergency, CS2 closed) drops the
//! open segment, the queue and any result still being computed.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{sync_channel, SyncSender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use super::engine::{default_threads, sanitize_threads, Recognizer};
use super::filter::prompt_with;
use super::segmenter::Segment;
use super::types::{
    EngineState, RejectReason, SpeechConfig, SpeechStats, SpeechStatus, Transcript, VoiceActivity,
};
use super::vad::VoiceDetector;
use super::SpeechError;
use crate::cpu::lower_current_thread_priority;

mod asr;
mod fallback;
mod front;
mod queue;
mod stats;

use asr::asr_loop;
use fallback::{is_light, Lag, LagWatch, FALLBACK_MODEL};
use front::front_loop;
pub use front::SpeechSink;
use queue::JobQueue;
use stats::StatsAcc;

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
    /// Signs that Whisper cannot keep up, during the current capture.
    lag: Mutex<LagWatch>,
    /// The chosen model while whisper-base replaces it (until the capture stops).
    fallback: Mutex<Option<String>>,
    /// whisper-base missing when a fallback was needed: said once.
    fallback_missing_said: AtomicBool,
}

impl Inner {
    /// The model that should be loaded: the chosen one, or whisper-base
    /// while it replaces it.
    fn wanted_model(&self, config: &SpeechConfig) -> String {
        match self.fallback.lock().unwrap().as_ref() {
            Some(_) => FALLBACK_MODEL.to_string(),
            None => config.model.clone(),
        }
    }

    /// The loaded model, if it may transcribe: the wanted one, or the chosen
    /// one while whisper-base is still loading to replace it.
    fn usable_model(&self, config: &SpeechConfig) -> Option<String> {
        let loaded = self.engine.lock().unwrap().loaded_model.clone()?;
        let falling_back = self.fallback.lock().unwrap().is_some();
        (loaded == self.wanted_model(config) || (falling_back && loaded == config.model))
            .then_some(loaded)
    }

    /// Records a sign that Whisper cannot keep up with a heavy model.
    fn note_lag(&self, lag: Lag) {
        if is_light(&self.config.lock().unwrap().model) || self.fallback.lock().unwrap().is_some() {
            return;
        }
        self.lag.lock().unwrap().note(lag, Instant::now());
    }

    /// Switches to whisper-base when the chosen model keeps lagging.
    fn check_fallback(self: &Arc<Self>) {
        let summary = {
            let mut lag = self.lag.lock().unwrap();
            if !lag.lagging() {
                return;
            }
            let summary = lag.summary();
            lag.clear();
            summary
        };
        let chosen = self.config.lock().unwrap().model.clone();
        if is_light(&chosen) || self.fallback.lock().unwrap().is_some() {
            return;
        }
        if self
            .loader
            .model_path(&self.models_dir, FALLBACK_MODEL)
            .is_none()
        {
            if !self.fallback_missing_said.swap(true, Ordering::SeqCst) {
                crate::logs::warn(
                    "asr",
                    &format!(
                        "whisper-{chosen} trop lent sur ce PC ({summary} en 1 min), et whisper-{FALLBACK_MODEL} n'est pas installé : installez-le (Audio & IA)"
                    ),
                );
            }
            return;
        }
        *self.fallback.lock().unwrap() = Some(chosen.clone());
        crate::logs::warn(
            "asr",
            &format!(
                "whisper-{chosen} trop lent sur ce PC ({summary} en 1 min) : bascule sur whisper-{FALLBACK_MODEL} jusqu'à l'arrêt de la capture"
            ),
        );
        load_models(self, None);
    }

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
        let wanted = self.wanted_model(&config);
        let recognizer_ready = e.loaded_model.as_deref() == Some(wanted.as_str());
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
            fallback_from: self.fallback.lock().unwrap().clone(),
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
            lag: Mutex::new(LagWatch::default()),
            fallback: Mutex::new(None),
            fallback_missing_said: AtomicBool::new(false),
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
            queue: JobQueue::default(),
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
            // An explicit choice ends any automatic fallback.
            *self.inner.fallback.lock().unwrap() = None;
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
        load_models(&self.inner, with_vad.then(|| self.front.clone()));
    }

    /// The capture stopped: back to the chosen model after a fallback.
    pub fn end_session(&self) {
        self.inner.lag.lock().unwrap().clear();
        let Some(chosen) = self.inner.fallback.lock().unwrap().take() else {
            return;
        };
        crate::logs::info(
            "asr",
            &format!("capture arrêtée : retour à whisper-{chosen}"),
        );
        self.reload(false);
    }

    /// Drops everything in flight: open segment, queued segments, running result.
    pub fn purge(&self) {
        let _publication = self.inner.publication.lock().unwrap();
        self.inner.generation.fetch_add(1, Ordering::SeqCst);
        self.inner.queue.clear();
        self.inner.lag.lock().unwrap().clear();
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

fn load_models(inner: &Arc<Inner>, front: Option<SyncSender<FrontMsg>>) {
    let inner = inner.clone();
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
            if let Some(front) = front {
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
                    (inner.wanted_model(&c), c.accel)
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
                if inner.wanted_model(&config) != model || config.accel != accel {
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

impl Drop for SpeechService {
    fn drop(&mut self) {
        self.inner.shutdown.store(true, Ordering::SeqCst);
    }
}

fn diagnostic_rejection(reason: RejectReason, audio_ms: u32, queue_ms: u128) {
    if std::env::var_os("CROSSTALK_SPEECH_DIAGNOSTICS").is_some_and(|v| v == "1") {
        eprintln!("crosstalk speech diagnostic: outcome={reason:?} audio_ms={audio_ms} queue_ms={queue_ms} inference=not_run raw=None");
    }
}

#[cfg(test)]
mod tests;
