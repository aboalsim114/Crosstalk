//! Capture service: owns the capture thread, applies the fallback plan, meters
//! the signal and reports every state change.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{sync_channel, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use super::backend::{CaptureBackend, CaptureStream, Packet};
use super::error::AudioError;
use super::level::LevelMeter;
use super::plan;
use super::types::{
    AudioBlock, AudioDevice, CaptureMode, CaptureState, CaptureStatus, CaptureTarget, LevelReading,
    SourceInfo, StopReason,
};

/// How long a read waits for audio; also bounds how fast `stop` takes effect.
const READ_TIMEOUT_MS: u32 = 20;
/// How long `start` waits for the capture thread to open a source.
const START_TIMEOUT: Duration = Duration::from_secs(5);
/// Reopen attempts after a recoverable error (device removed, audio service restarted).
const MAX_REOPENS: u32 = 3;
const REOPEN_DELAY: Duration = Duration::from_millis(300);

/// Receives state changes and level readings (the Tauri event emitter in the app).
pub trait AudioEvents: Send + Sync + 'static {
    fn status(&self, status: &CaptureStatus);
    fn level(&self, reading: LevelReading);
}

/// Consumer of the raw captured audio, called on the capture thread: it must
/// return quickly. Used by the diagnostic probe today, by speech recognition later.
pub trait BlockSink: Send {
    fn block(&mut self, block: &AudioBlock);
    fn silence(&mut self, _frames: usize, _channels: u16, _sample_rate: u32) {}
}

type SharedSink = Arc<Mutex<Option<Box<dyn BlockSink>>>>;

struct Worker {
    stop: Arc<AtomicBool>,
    handle: JoinHandle<()>,
}

struct Inner {
    backend: Arc<dyn CaptureBackend>,
    events: Arc<dyn AudioEvents>,
    status: Mutex<CaptureStatus>,
    sink: SharedSink,
}

impl Inner {
    fn publish(&self, status: CaptureStatus) {
        *self.status.lock().unwrap() = status.clone();
        self.events.status(&status);
    }
}

pub struct AudioService {
    inner: Arc<Inner>,
    worker: Mutex<Option<Worker>>,
}

impl AudioService {
    pub fn new(backend: Arc<dyn CaptureBackend>, events: Arc<dyn AudioEvents>) -> Self {
        Self {
            inner: Arc::new(Inner {
                backend,
                events,
                status: Mutex::new(CaptureStatus::idle()),
                sink: Arc::new(Mutex::new(None)),
            }),
            worker: Mutex::new(None),
        }
    }

    pub fn status(&self) -> CaptureStatus {
        self.inner.status.lock().unwrap().clone()
    }

    pub fn devices(&self) -> Result<Vec<AudioDevice>, AudioError> {
        self.inner.backend.list_devices()
    }

    pub fn set_sink(&self, sink: Option<Box<dyn BlockSink>>) {
        *self.inner.sink.lock().unwrap() = sink;
    }

    /// Starts (or restarts) the capture and returns once a source is open, the
    /// plan failed, or the start timed out.
    pub fn start(
        &self,
        mode: CaptureMode,
        device_id: Option<String>,
        cs2_pid: Option<u32>,
    ) -> CaptureStatus {
        let mut worker = self.worker.lock().unwrap();
        if let Some(w) = worker.take() {
            join(w);
        }

        let targets = match plan::attempts(mode, device_id.as_deref(), cs2_pid) {
            Ok(t) => t,
            Err(e) => {
                self.inner.publish(error_status(mode, None, &e));
                return self.status();
            }
        };
        self.inner.publish(CaptureStatus {
            state: CaptureState::Starting,
            mode: Some(mode),
            ..CaptureStatus::idle()
        });

        let stop = Arc::new(AtomicBool::new(false));
        let (ready_tx, ready_rx) = sync_channel::<()>(1);
        let inner = self.inner.clone();
        let stop_flag = stop.clone();
        let handle = std::thread::Builder::new()
            .name("audio-capture".into())
            .spawn(move || {
                run(&inner, mode, &targets, &stop_flag, || {
                    let _ = ready_tx.send(());
                })
            })
            .expect("spawn audio capture thread");

        match ready_rx.recv_timeout(START_TIMEOUT) {
            // Opened, or failed and published its error; either way `ready` fired.
            Ok(()) | Err(RecvTimeoutError::Disconnected) => {}
            Err(RecvTimeoutError::Timeout) => {
                stop.store(true, Ordering::SeqCst);
                self.inner.publish(error_status(
                    mode,
                    None,
                    &AudioError::Timeout("ouverture de la source audio"),
                ));
            }
        }
        *worker = Some(Worker { stop, handle });
        self.status()
    }

    /// Stops the capture and drops any buffered audio. Returns within about
    /// two read timeouts (well under the 200 ms required for the emergency stop).
    pub fn stop(&self, reason: StopReason) -> CaptureStatus {
        if let Some(w) = self.worker.lock().unwrap().take() {
            join(w);
        }
        let mode = self.status().mode;
        self.inner.publish(CaptureStatus {
            mode,
            stop_reason: Some(reason),
            ..CaptureStatus::idle()
        });
        self.status()
    }

    /// Called by the CS2 watcher: a process capture of an exited game is stopped.
    pub fn on_process_exited(&self, pid: u32) {
        let capturing_it = matches!(
            self.status().source,
            Some(SourceInfo::Process { pid: p, .. }) if p == pid
        );
        if capturing_it {
            self.stop(StopReason::Cs2Exited);
        }
    }
}

fn join(w: Worker) {
    w.stop.store(true, Ordering::SeqCst);
    let _ = w.handle.join();
}

fn error_status(mode: CaptureMode, source: Option<SourceInfo>, e: &AudioError) -> CaptureStatus {
    CaptureStatus {
        state: CaptureState::Error,
        mode: Some(mode),
        source,
        error: Some(e.info()),
        ..CaptureStatus::idle()
    }
}

/// Body of the capture thread.
fn run(
    inner: &Inner,
    mode: CaptureMode,
    targets: &[CaptureTarget],
    stop: &AtomicBool,
    ready: impl FnOnce(),
) {
    // Try each target in order; remember why the preferred one failed.
    let mut first_error: Option<AudioError> = None;
    let mut opened: Option<(CaptureTarget, Box<dyn CaptureStream>)> = None;
    let mut last_error = AudioError::NoDevice;
    for target in targets {
        match inner.backend.open(target) {
            Ok(stream) => {
                opened = Some((target.clone(), stream));
                break;
            }
            Err(e) => {
                first_error.get_or_insert_with(|| e.clone());
                last_error = e;
            }
        }
    }
    let Some((target, mut stream)) = opened else {
        inner.publish(error_status(mode, None, &last_error));
        ready();
        return;
    };
    let fallback_reason = match (&target, &targets[0]) {
        (CaptureTarget::Device { .. }, CaptureTarget::Process { .. }) => {
            first_error.map(|e| e.to_string())
        }
        _ => None,
    };
    let capturing = |stream: &dyn CaptureStream| CaptureStatus {
        state: CaptureState::Capturing,
        mode: Some(mode),
        source: Some(stream.source()),
        format: Some(stream.format()),
        fallback_reason: fallback_reason.clone(),
        ..CaptureStatus::idle()
    };
    inner.publish(capturing(stream.as_ref()));
    ready();

    let mut meter = LevelMeter::new();
    let mut reopens = 0;
    while !stop.load(Ordering::SeqCst) {
        let format = stream.format();
        let result = stream.read(READ_TIMEOUT_MS, &mut |packet| {
            let mut sink = inner.sink.lock().unwrap();
            match packet {
                Packet::Audio(block) => {
                    meter.push(&block, |r| inner.events.level(r));
                    if let Some(s) = sink.as_mut() {
                        s.block(&block);
                    }
                }
                Packet::Silence { frames, .. } => {
                    meter.push_silence(frames, format.sample_rate, |r| inner.events.level(r));
                    if let Some(s) = sink.as_mut() {
                        s.silence(frames, format.channels, format.sample_rate);
                    }
                }
            }
        });
        let Err(e) = result else { continue };

        let source = Some(stream.source());
        if e.is_recoverable() && reopens < MAX_REOPENS {
            drop(stream);
            if !sleep_unless_stopped(REOPEN_DELAY, stop) {
                return;
            }
            match inner.backend.open(&target) {
                Ok(s) => {
                    reopens += 1;
                    stream = s;
                    inner.publish(capturing(stream.as_ref()));
                    continue;
                }
                Err(e2) => {
                    inner.publish(error_status(mode, source, &e2));
                    return;
                }
            }
        }
        inner.publish(error_status(mode, source, &e));
        return;
    }
}

/// Sleeps in small steps; returns false if a stop was requested meanwhile.
fn sleep_unless_stopped(total: Duration, stop: &AtomicBool) -> bool {
    let end = Instant::now() + total;
    while Instant::now() < end {
        if stop.load(Ordering::SeqCst) {
            return false;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    !stop.load(Ordering::SeqCst)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::types::{SampleFormat, StreamFormat};
    use std::sync::atomic::AtomicUsize;

    const RATE: u32 = 48_000;
    const AMPLITUDE: f32 = 0.5;

    #[derive(Default)]
    struct Recorder {
        statuses: Mutex<Vec<CaptureStatus>>,
        levels: Mutex<Vec<LevelReading>>,
    }
    impl AudioEvents for Recorder {
        fn status(&self, s: &CaptureStatus) {
            self.statuses.lock().unwrap().push(s.clone());
        }
        fn level(&self, r: LevelReading) {
            self.levels.lock().unwrap().push(r);
        }
    }

    #[derive(Default)]
    struct FakeBackend {
        process_error: Option<AudioError>,
        device_error: Option<AudioError>,
        /// Make the first stream fail with DeviceInvalidated after this many reads.
        invalidate_after: Option<usize>,
        reopen_error: Option<AudioError>,
        opens: AtomicUsize,
    }

    struct FakeStream {
        source: SourceInfo,
        reads: usize,
        fail_after: Option<usize>,
        buf: Vec<f32>,
    }

    impl CaptureStream for FakeStream {
        fn format(&self) -> StreamFormat {
            StreamFormat {
                sample_rate: RATE,
                channels: 2,
                sample_format: SampleFormat::F32,
            }
        }
        fn source(&self) -> SourceInfo {
            self.source.clone()
        }
        fn read(
            &mut self,
            _timeout: u32,
            on: &mut dyn FnMut(Packet<'_>),
        ) -> Result<(), AudioError> {
            std::thread::sleep(Duration::from_millis(2));
            self.reads += 1;
            if self.fail_after.is_some_and(|n| self.reads > n) {
                return Err(AudioError::DeviceInvalidated);
            }
            // 10 ms of a stereo square wave at ±AMPLITUDE.
            on(Packet::Audio(AudioBlock {
                samples: &self.buf,
                channels: 2,
                sample_rate: RATE,
                qpc_100ns: 0,
            }));
            Ok(())
        }
    }

    impl CaptureBackend for FakeBackend {
        fn list_devices(&self) -> Result<Vec<AudioDevice>, AudioError> {
            Ok(vec![AudioDevice {
                id: "dev".into(),
                name: "Casque".into(),
                is_default: true,
            }])
        }
        fn open(&self, target: &CaptureTarget) -> Result<Box<dyn CaptureStream>, AudioError> {
            let n = self.opens.fetch_add(1, Ordering::SeqCst);
            let (source, err) = match target {
                CaptureTarget::Process { pid } => (
                    SourceInfo::Process {
                        pid: *pid,
                        process_name: "cs2.exe".into(),
                    },
                    self.process_error.clone(),
                ),
                CaptureTarget::Device { id } => (
                    SourceInfo::Device {
                        id: id.clone().unwrap_or("dev".into()),
                        name: "Casque".into(),
                    },
                    self.device_error.clone(),
                ),
            };
            if let Some(e) = err {
                return Err(e);
            }
            if n > 0 && self.invalidate_after.is_some() {
                if let Some(e) = self.reopen_error.clone() {
                    return Err(e);
                }
            }
            let buf = (0..480)
                .flat_map(|i| {
                    let v = if i % 2 == 0 { AMPLITUDE } else { -AMPLITUDE };
                    [v, v]
                })
                .collect();
            Ok(Box::new(FakeStream {
                source,
                reads: 0,
                fail_after: if n == 0 { self.invalidate_after } else { None },
                buf,
            }))
        }
    }

    fn service(backend: FakeBackend) -> (AudioService, Arc<Recorder>) {
        let rec = Arc::new(Recorder::default());
        (AudioService::new(Arc::new(backend), rec.clone()), rec)
    }

    #[test]
    fn auto_mode_captures_cs2_and_meters_the_signal() {
        let (svc, rec) = service(FakeBackend::default());
        let st = svc.start(CaptureMode::Auto, None, Some(1234));
        assert_eq!(st.state, CaptureState::Capturing);
        assert_eq!(
            st.source,
            Some(SourceInfo::Process {
                pid: 1234,
                process_name: "cs2.exe".into()
            })
        );
        assert_eq!(st.fallback_reason, None);
        std::thread::sleep(Duration::from_millis(150));
        svc.stop(StopReason::User);
        let levels = rec.levels.lock().unwrap();
        assert!(!levels.is_empty());
        // A ±0.5 square wave has an RMS of 0.5 (−6 dB).
        assert!(
            (levels[0].rms_db + 6.02).abs() < 0.1,
            "{}",
            levels[0].rms_db
        );
        let states: Vec<_> = rec
            .statuses
            .lock()
            .unwrap()
            .iter()
            .map(|s| s.state)
            .collect();
        assert_eq!(
            states,
            vec![
                CaptureState::Starting,
                CaptureState::Capturing,
                CaptureState::Idle
            ]
        );
    }

    #[test]
    fn falls_back_to_the_device_and_says_why() {
        let (svc, _) = service(FakeBackend {
            process_error: Some(AudioError::ProcessLoopbackUnavailable("E_NOTIMPL".into())),
            ..Default::default()
        });
        let st = svc.start(CaptureMode::Auto, None, Some(9));
        assert_eq!(st.state, CaptureState::Capturing);
        assert!(matches!(st.source, Some(SourceInfo::Device { .. })));
        assert!(st.fallback_reason.unwrap().contains("E_NOTIMPL"));
        svc.stop(StopReason::User);
    }

    #[test]
    fn reports_the_last_error_when_every_source_fails() {
        let (svc, _) = service(FakeBackend {
            process_error: Some(AudioError::ProcessLoopbackUnavailable("x".into())),
            device_error: Some(AudioError::NoDevice),
            ..Default::default()
        });
        let st = svc.start(CaptureMode::Auto, None, Some(9));
        assert_eq!(st.state, CaptureState::Error);
        assert_eq!(st.error.unwrap().code, "no_device");
    }

    #[test]
    fn auto_without_cs2_does_not_capture_the_whole_device() {
        let (svc, _) = service(FakeBackend::default());
        let st = svc.start(CaptureMode::Auto, None, None);
        assert_eq!(st.state, CaptureState::Error);
        assert_eq!(st.error.unwrap().code, "cs2_not_running");
        assert_eq!(svc.inner.backend.list_devices().unwrap().len(), 1);
    }

    #[test]
    fn stop_is_fast_enough_for_the_emergency_stop() {
        let (svc, _) = service(FakeBackend::default());
        svc.start(CaptureMode::Device, None, None);
        let t = Instant::now();
        let st = svc.stop(StopReason::Emergency);
        assert!(
            t.elapsed() < Duration::from_millis(200),
            "{:?}",
            t.elapsed()
        );
        assert_eq!(st.state, CaptureState::Idle);
        assert_eq!(st.stop_reason, Some(StopReason::Emergency));
    }

    #[test]
    fn stops_when_the_captured_game_exits() {
        let (svc, _) = service(FakeBackend::default());
        svc.start(CaptureMode::Process, None, Some(5));
        svc.on_process_exited(6);
        assert_eq!(svc.status().state, CaptureState::Capturing);
        svc.on_process_exited(5);
        let st = svc.status();
        assert_eq!(st.state, CaptureState::Idle);
        assert_eq!(st.stop_reason, Some(StopReason::Cs2Exited));
    }

    #[test]
    fn reopens_after_the_device_is_invalidated() {
        let (svc, rec) = service(FakeBackend {
            invalidate_after: Some(3),
            ..Default::default()
        });
        svc.start(CaptureMode::Device, None, None);
        std::thread::sleep(Duration::from_millis(500));
        assert_eq!(svc.status().state, CaptureState::Capturing);
        let before = rec.levels.lock().unwrap().len();
        std::thread::sleep(Duration::from_millis(150));
        assert!(
            rec.levels.lock().unwrap().len() > before,
            "levels resume after reopen"
        );
        svc.stop(StopReason::User);
    }

    #[test]
    fn reports_device_loss_when_reopening_fails() {
        let (svc, _) = service(FakeBackend {
            invalidate_after: Some(1),
            reopen_error: Some(AudioError::DeviceInvalidated),
            ..Default::default()
        });
        svc.start(CaptureMode::Device, None, None);
        std::thread::sleep(Duration::from_millis(500));
        let st = svc.status();
        assert_eq!(st.state, CaptureState::Error);
        assert_eq!(st.error.unwrap().code, "device_invalidated");
        svc.stop(StopReason::User);
    }

    #[test]
    fn restarting_replaces_the_running_capture() {
        let backend = FakeBackend::default();
        let (svc, _) = service(backend);
        svc.start(CaptureMode::Device, Some("a".into()), None);
        let st = svc.start(CaptureMode::Device, Some("b".into()), None);
        assert!(matches!(st.source, Some(SourceInfo::Device { ref id, .. }) if id == "b"));
        svc.stop(StopReason::User);
    }

    #[test]
    fn sink_receives_the_captured_blocks() {
        struct Count(Arc<AtomicUsize>);
        impl BlockSink for Count {
            fn block(&mut self, b: &AudioBlock) {
                self.0.fetch_add(b.frames(), Ordering::SeqCst);
            }
        }
        let frames = Arc::new(AtomicUsize::new(0));
        let (svc, _) = service(FakeBackend::default());
        svc.set_sink(Some(Box::new(Count(frames.clone()))));
        svc.start(CaptureMode::Device, None, None);
        std::thread::sleep(Duration::from_millis(60));
        svc.stop(StopReason::User);
        assert!(frames.load(Ordering::SeqCst) >= 480);
    }
}
