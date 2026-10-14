use super::asr::{finalize_from_live, judge_current, transcribe};
use super::front::FrameClock;
use super::queue::Jobs;
use super::*;
use crate::audio::{AudioBlock, BlockSink};
use crate::speech::engine::RecognizeOptions;
use crate::speech::filter::Recognition;
use crate::speech::types::Lang;
use crate::speech::vad::EnergyDetector;
use crate::speech::vad::FRAME;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Condvar;

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
