//! "Voix IA": the user dictates a sentence in their language (Mouse 5),
//! Crosstalk transcribes and translates it (src/dictation.rs) and a local
//! neural voice (Piper) says it in the teammates' language, in CS2's voice
//! chat, through a virtual cable (VB-CABLE). Off by default; synthesis runs on
//! the PC, no key, no account. With "PTT automatique" (ptt.rs) Crosstalk holds
//! CS2's voice key while the sentence plays.

pub mod catalog;
pub mod mix;
pub mod piper;
pub mod ptt;
pub mod router;
pub mod wav;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};

use crate::downloads::{DownloadEvents, DownloadStatus, Downloader};

pub const DOWNLOAD_EVENT: &str = "voice://download";
pub const STATE_EVENT: &str = "voice://state";
pub const PTT_EVENT: &str = "voice://ptt";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct VoiceOptions {
    pub enabled: bool,
    /// The user's real microphone (its id), passed through to the cable.
    pub mic: Option<String>,
    /// Chosen voice per language (`ru` → `ru_RU-dmitri-medium`).
    pub voices: HashMap<String, String>,
    /// Also hear the AI voice in the headset.
    pub monitor: bool,
    /// Volumes in percent (0–150).
    pub volume: u32,
    pub mic_volume: u32,
    /// Hold CS2's voice key while the AI voice plays (off by default).
    pub auto_ptt: bool,
    /// The key bound to voice chat in CS2 ("K", "V", "Mouse4"…).
    pub ptt_key: String,
    /// Key held before the first sample, and after the last one (ms).
    pub ptt_lead_ms: u32,
    pub ptt_tail_ms: u32,
    /// Also say the replies typed in the pop-up (Mouse 4) when Enter is pressed.
    pub speak_typed: bool,
}

impl Default for VoiceOptions {
    fn default() -> Self {
        Self {
            enabled: false,
            mic: None,
            voices: HashMap::new(),
            monitor: true,
            volume: 100,
            mic_volume: 100,
            auto_ptt: false,
            ptt_key: "K".into(),
            ptt_lead_ms: ptt::DEFAULT_LEAD_MS,
            ptt_tail_ms: ptt::DEFAULT_TAIL_MS,
            speak_typed: false,
        }
    }
}

impl VoiceOptions {
    /// What a transmission does with these settings (an unknown key is an error).
    pub fn plan(&self) -> Result<ptt::Plan, String> {
        ptt::Plan::new(
            self.auto_ptt,
            &self.ptt_key,
            self.ptt_lead_ms,
            self.ptt_tail_ms,
        )
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceInfo {
    #[serde(flatten)]
    pub voice: catalog::Voice,
    pub installed: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceStatus {
    pub options: VoiceOptions,
    pub engine_ready: bool,
    pub voices: Vec<VoiceInfo>,
    /// VB-CABLE's input, when installed.
    pub cable: Option<crate::audio::AudioDevice>,
    pub mics: Vec<crate::audio::AudioDevice>,
    /// The cable is fed (microphone passthrough running).
    pub routing: bool,
    pub speaking: bool,
    pub error: Option<String>,
    pub download: DownloadStatus,
    /// Last automatic push-to-talk state.
    pub ptt: ptt::PttStatus,
}

/// What the overlay and the reply window show while the voice speaks.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpeakState {
    pub text: String,
    pub lang: String,
    pub ms: u32,
}

/// Test sentence per language (the preview button).
pub fn sample_sentence(lang: &str) -> &'static str {
    match lang {
        "ru" => "Двое на B, осторожно, флешка.",
        "en" => "Two on B, watch out for the flash.",
        "fr" => "Deux en B, attention au flash.",
        "de" => "Zwei auf B, Vorsicht, Blendgranate.",
        "es" => "Dos en B, cuidado con la flash.",
        "pl" => "Dwóch na B, uwaga na flasha.",
        "tr" => "B'de iki kişi, flaşa dikkat.",
        "uk" => "Двоє на B, обережно, флешка.",
        _ => "Two on B.",
    }
}

/// One sentence waiting for the push-to-talk worker.
struct Job {
    samples: Vec<f32>,
    rate: u32,
    plan: ptt::Plan,
    feed: Arc<router::Feed>,
    generation: u64,
}

type PttListener = Box<dyn Fn(&ptt::PttStatus) + Send + Sync>;

/// Shared with the worker thread.
struct Ptt {
    keys: Arc<dyn ptt::Keys>,
    held: Arc<Mutex<Option<ptt::PttKey>>>,
    generation: ptt::Generation,
    status: Mutex<ptt::PttStatus>,
    listener: Mutex<Option<PttListener>>,
}

impl Ptt {
    fn report(&self, s: ptt::PttStatus) {
        *self.status.lock().unwrap() = s.clone();
        if let Some(l) = self.listener.lock().unwrap().as_ref() {
            l(&s);
        }
    }

    fn run(&self, job: Job) {
        if job.generation != self.generation.current() {
            return self.report(ptt::PttStatus::at(ptt::Phase::Cancelled));
        }
        let cancelled = || self.generation.current() != job.generation;
        let end = ptt::transmit(
            job.plan,
            &job.samples,
            job.rate,
            &self.keys,
            &*job.feed,
            &self.held,
            &cancelled,
            &mut |s| self.report(s),
        );
        if let Some(d) = &end.detail {
            crate::logs::warn("voice", &format!("PTT automatique : {d}"));
        }
    }
}

pub struct VoiceService {
    models: PathBuf,
    options: Mutex<VoiceOptions>,
    piper: Mutex<Option<Arc<piper::Piper>>>,
    router: Mutex<Option<router::Router>>,
    error: Arc<Mutex<Option<String>>>,
    downloader: Arc<Downloader>,
    ptt: Arc<Ptt>,
    /// Sentences played one after the other, never on top of each other.
    jobs: Mutex<Option<mpsc::Sender<Job>>>,
}

impl VoiceService {
    pub fn new(models: PathBuf, downloader: Arc<Downloader>) -> Arc<Self> {
        Self::with_keys(models, downloader, Arc::new(ptt::WindowsKeys::new()))
    }

    pub fn with_keys(
        models: PathBuf,
        downloader: Arc<Downloader>,
        keys: Arc<dyn ptt::Keys>,
    ) -> Arc<Self> {
        Arc::new(Self {
            models,
            options: Mutex::new(VoiceOptions::default()),
            piper: Mutex::new(None),
            router: Mutex::new(None),
            error: Arc::new(Mutex::new(None)),
            downloader,
            ptt: Arc::new(Ptt {
                keys,
                held: Arc::new(Mutex::new(None)),
                generation: ptt::Generation::default(),
                status: Mutex::new(ptt::PttStatus::at(ptt::Phase::Idle)),
                listener: Mutex::new(None),
            }),
            jobs: Mutex::new(None),
        })
    }

    /// Called with each push-to-talk state (the windows).
    pub fn on_ptt(&self, f: impl Fn(&ptt::PttStatus) + Send + Sync + 'static) {
        *self.ptt.listener.lock().unwrap() = Some(Box::new(f));
    }

    pub fn ptt_status(&self) -> ptt::PttStatus {
        self.ptt.status.lock().unwrap().clone()
    }

    /// Crosstalk is holding CS2's voice key right now (its presses are not
    /// the user's: see tap_keys.rs).
    #[cfg_attr(not(windows), allow(dead_code))]
    pub fn holding_key(&self) -> bool {
        self.ptt.held.lock().unwrap().is_some()
    }

    /// Stops the sentence playing and the queued ones; the key is released at once.
    pub fn cancel(&self) {
        self.ptt.generation.bump();
        ptt::release_held(self.ptt.keys.as_ref(), &self.ptt.held);
        if let Some(r) = self.router.lock().unwrap().as_ref() {
            r.feed.stop_voice();
        }
    }

    /// Crosstalk closing: never leave CS2's voice key held.
    pub fn shutdown(&self) {
        self.cancel();
    }

    fn enqueue(&self, job: Job) -> Result<(), String> {
        let mut jobs = self.jobs.lock().unwrap();
        if jobs.is_none() {
            let (tx, rx) = mpsc::channel::<Job>();
            let ptt = self.ptt.clone();
            std::thread::Builder::new()
                .name("voice-ptt".into())
                .spawn(move || {
                    for job in rx {
                        ptt.run(job);
                    }
                })
                .map_err(|e| e.to_string())?;
            *jobs = Some(tx);
        }
        jobs.as_ref()
            .unwrap()
            .send(job)
            .map_err(|_| "file de la voix IA fermée".to_string())
    }

    pub fn options(&self) -> VoiceOptions {
        self.options.lock().unwrap().clone()
    }

    /// The voice for a language: the chosen one, else the first of the catalogue.
    pub fn voice_for(&self, lang: &str) -> Option<&'static catalog::Voice> {
        let chosen = self.options.lock().unwrap().voices.get(lang).cloned();
        chosen
            .and_then(|id| catalog::voice(&id).filter(|v| v.lang == lang))
            .or_else(|| catalog::default_voice(lang))
    }

    pub fn status(&self) -> VoiceStatus {
        let outputs = crate::audio::list_endpoints(false).unwrap_or_default();
        let mics = crate::audio::list_endpoints(true).unwrap_or_default();
        let cable = router::find_cable(&outputs).cloned();
        let options = self.options();
        let wanted: Vec<String> = options.voices.values().cloned().collect();
        let missing = catalog::missing(&self.models, &wanted);
        let (routing, speaking) = self
            .router
            .lock()
            .unwrap()
            .as_ref()
            .map_or((false, false), |r| (true, r.feed.speaking()));
        VoiceStatus {
            engine_ready: catalog::engine_ready(&self.models),
            voices: catalog::VOICES
                .iter()
                .map(|v| VoiceInfo {
                    voice: *v,
                    installed: catalog::voice_installed(&self.models, v),
                })
                .collect(),
            cable,
            // The cable's recording side is not a real microphone.
            mics: mics
                .into_iter()
                .filter(|m| !m.name.to_lowercase().contains("cable output"))
                .collect(),
            routing,
            speaking,
            error: self.error.lock().unwrap().clone(),
            download: self.downloader.snapshot(&missing),
            ptt: self.ptt_status(),
            options,
        }
    }

    /// Applies new settings: starts, restarts or stops the cable feed.
    pub fn configure(&self, opts: VoiceOptions) {
        let route = if opts.enabled {
            let outputs = crate::audio::list_endpoints(false).unwrap_or_default();
            router::find_cable(&outputs).map(|c| router::RouteConfig {
                mic: opts.mic.clone(),
                cable: c.id.clone(),
                monitor: opts.monitor,
            })
        } else {
            None
        };
        let before = std::mem::replace(&mut *self.options.lock().unwrap(), opts.clone());
        // Any change to what the key or the cable does stops a transmission (key released).
        let ptt_changed = (
            before.enabled,
            before.auto_ptt,
            &before.ptt_key,
            &before.mic,
            before.monitor,
        ) != (
            opts.enabled,
            opts.auto_ptt,
            &opts.ptt_key,
            &opts.mic,
            opts.monitor,
        );
        if ptt_changed {
            self.cancel();
        }
        let mut r = self.router.lock().unwrap();
        let same = matches!((&*r, &route), (Some(cur), Some(want)) if cur.config == *want);
        if !same {
            if let Some(old) = r.take() {
                old.stop();
            }
            *self.error.lock().unwrap() = None;
            if let Some(cfg) = route {
                let err = self.error.clone();
                match router::start(cfg, move |e| *err.lock().unwrap() = Some(e)) {
                    Ok(router) => *r = Some(router),
                    Err(e) => *self.error.lock().unwrap() = Some(e),
                }
            } else if opts.enabled {
                *self.error.lock().unwrap() =
                    Some("VB-CABLE introuvable : installez-le, puis relancez la détection.".into());
            }
        }
        if let Some(router) = r.as_ref() {
            *router.feed.mic_gain.lock().unwrap() = mix::gain(opts.mic_volume);
            *router.feed.voice_gain.lock().unwrap() = mix::gain(opts.volume);
        }
        if !opts.enabled {
            if let Some(p) = self.piper.lock().unwrap().as_ref() {
                p.stop();
            }
        }
    }

    fn engine(&self) -> Result<Arc<piper::Piper>, String> {
        let mut p = self.piper.lock().unwrap();
        if let Some(p) = p.as_ref() {
            return Ok(p.clone());
        }
        let exe = catalog::extract_engine(&self.models)?;
        let engine = Arc::new(piper::Piper::new(
            exe,
            catalog::dir(&self.models).join("tmp"),
        ));
        *p = Some(engine.clone());
        Ok(engine)
    }

    /// Synthesises `text` with the language's voice.
    pub fn synthesize(&self, text: &str, lang: &str) -> Result<wav::Pcm, String> {
        let v = self
            .voice_for(lang)
            .ok_or_else(|| format!("aucune voix pour la langue « {lang} »"))?;
        if !catalog::voice_installed(&self.models, v) {
            return Err(format!("voix {} non téléchargée", v.name));
        }
        self.engine()?
            .say(&catalog::model_path(&self.models, v), text)
    }

    /// Says `text` in the game (through the cable). Returns its length in ms.
    /// The sentence is fully synthesised before anything is pressed; with
    /// "PTT automatique" it is then played by the worker, key held.
    pub fn say(&self, text: &str, lang: &str) -> Result<u32, String> {
        let opts = self.options();
        let fail = |e: String| {
            if opts.auto_ptt {
                self.ptt.report(ptt::PttStatus {
                    phase: ptt::Phase::Error,
                    detail: Some(e.clone()),
                    pressed: false,
                });
            }
            e
        };
        // Checked before the synthesis: a wrong key is said at once.
        let plan = opts.plan().map_err(fail)?;
        if opts.auto_ptt {
            self.ptt.report(ptt::PttStatus::at(ptt::Phase::Preparing));
        }
        let generation = self.ptt.generation.current();
        let pcm = self.synthesize(text, lang).map_err(fail)?;
        let feed = {
            let r = self.router.lock().unwrap();
            let router = r.as_ref().ok_or_else(|| {
                fail(
                    self.error
                        .lock()
                        .unwrap()
                        .clone()
                        .unwrap_or_else(|| "voix IA désactivée".into()),
                )
            })?;
            router.feed.clone()
        };
        let ms = (pcm.samples.len() as u64 * 1000 / pcm.rate.max(1) as u64) as u32;
        if plan.key.is_none() {
            feed.speak(&pcm.samples, pcm.rate);
            return Ok(ms);
        }
        self.enqueue(Job {
            samples: pcm.samples,
            rate: pcm.rate,
            plan,
            feed,
            generation,
        })?;
        Ok(ms)
    }

    /// Plays the test sentence of a voice in the headset only.
    pub fn preview(&self, voice_id: &str) -> Result<u32, String> {
        let v = catalog::voice(voice_id).ok_or("voix inconnue")?;
        if !catalog::voice_installed(&self.models, v) {
            return Err(format!("voix {} non téléchargée", v.name));
        }
        let pcm = self.engine()?.say(
            &catalog::model_path(&self.models, v),
            sample_sentence(v.lang),
        )?;
        let ms = (pcm.samples.len() as u64 * 1000 / pcm.rate.max(1) as u64) as u32;
        router::preview(pcm.samples, pcm.rate)?;
        Ok(ms)
    }

    pub fn download(&self, voice_ids: &[String]) -> DownloadStatus {
        let files = catalog::missing(&self.models, voice_ids);
        self.downloader.start_files(files.clone());
        self.downloader.snapshot(&files)
    }

    pub fn cancel_download(&self) {
        self.downloader.cancel();
    }
}

/// Download progress to the windows; the engine is unpacked once complete.
pub struct VoiceDownloadEvents(pub AppHandle);

impl DownloadEvents for VoiceDownloadEvents {
    fn progress(&self, status: &DownloadStatus) {
        let _ = self.0.emit(DOWNLOAD_EVENT, status);
    }
    fn finished(&self) {
        if let Err(e) = catalog::extract_engine(&crate::paths::models_dir()) {
            crate::logs::warn("voice", &format!("moteur de voix : {e}"));
        }
    }
}

/// Builds the service and its own downloader (separate from the models').
pub fn start(app: &AppHandle) -> Arc<VoiceService> {
    let downloader = Downloader::new(
        Arc::new(crate::downloads::HttpFetcher),
        Arc::new(VoiceDownloadEvents(app.clone())),
    );
    let v = VoiceService::new(crate::paths::models_dir(), downloader);
    let handle = app.clone();
    v.on_ptt(move |s| {
        let _ = handle.emit(PTT_EVENT, s);
    });
    v
}

/// The AI voice is on.
pub fn enabled(app: &AppHandle) -> bool {
    app.try_state::<Arc<VoiceService>>()
        .is_some_and(|v| v.options().enabled)
}

/// The AI voice is on and holds CS2's voice key: CS2 must be in front.
pub fn presses_key(app: &AppHandle) -> bool {
    app.try_state::<Arc<VoiceService>>().is_some_and(|v| {
        let o = v.options();
        o.enabled && o.auto_ptt
    })
}

/// Says a reply in the game when the AI voice is on; logs the outcome.
pub fn say_reply(app: &AppHandle, text: &str, lang: &str) -> Result<u32, String> {
    let v = app
        .try_state::<Arc<VoiceService>>()
        .ok_or("voix IA indisponible")?;
    if !v.options().enabled {
        return Err("voix IA désactivée (Audio & IA › Voix IA)".into());
    }
    match v.say(text, lang) {
        Ok(ms) => {
            let _ = app.emit(
                STATE_EVENT,
                SpeakState {
                    text: text.into(),
                    lang: lang.into(),
                    ms,
                },
            );
            crate::logs::info("voice", &format!("réponse dite en jeu ({lang}, {ms} ms)"));
            Ok(ms)
        }
        Err(e) => {
            crate::logs::warn("voice", &format!("voix IA : {e}"));
            Err(e)
        }
    }
}

// ---------- commands (async: device listing and synthesis are slow) ----------

#[tauri::command(async)]
pub fn voice_status(v: tauri::State<'_, Arc<VoiceService>>) -> VoiceStatus {
    v.status()
}

#[tauri::command(async)]
pub fn voice_configure(
    v: tauri::State<'_, Arc<VoiceService>>,
    options: VoiceOptions,
) -> VoiceStatus {
    v.configure(options);
    v.status()
}

#[tauri::command(async)]
pub fn voice_download(
    v: tauri::State<'_, Arc<VoiceService>>,
    voices: Vec<String>,
) -> DownloadStatus {
    v.download(&voices)
}

#[tauri::command]
pub fn voice_download_cancel(v: tauri::State<'_, Arc<VoiceService>>) {
    v.cancel_download();
}

#[tauri::command(async)]
pub fn voice_say(
    app: AppHandle,
    v: tauri::State<'_, Arc<VoiceService>>,
    text: String,
    lang: String,
) -> Result<u32, String> {
    let ms = v.say(&text, &lang)?;
    let _ = app.emit(STATE_EVENT, SpeakState { text, lang, ms });
    Ok(ms)
}

/// Stops the AI voice now (the push-to-talk key is released at once).
#[tauri::command]
pub fn voice_cancel(v: tauri::State<'_, Arc<VoiceService>>) {
    v.cancel();
}

/// Reproducible test: waits `delay_s` (time to switch to CS2), then says the
/// test sentence of `lang` in the game with the current settings.
#[tauri::command(async)]
pub fn voice_ptt_test(
    app: AppHandle,
    v: tauri::State<'_, Arc<VoiceService>>,
    lang: String,
    delay_s: u32,
) -> Result<u32, String> {
    let generation = v.ptt.generation.current();
    v.ptt.report(ptt::PttStatus {
        phase: ptt::Phase::Preparing,
        detail: Some(format!("test dans {} s : passez sur CS2", delay_s.min(10))),
        pressed: false,
    });
    let until = std::time::Instant::now() + std::time::Duration::from_secs(delay_s.min(10) as u64);
    while std::time::Instant::now() < until {
        if v.ptt.generation.current() != generation {
            v.ptt.report(ptt::PttStatus::at(ptt::Phase::Cancelled));
            return Err("test annulé".into());
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    let text = sample_sentence(&lang);
    let ms = v.say(text, &lang)?;
    let _ = app.emit(
        STATE_EVENT,
        SpeakState {
            text: text.into(),
            lang,
            ms,
        },
    );
    Ok(ms)
}

#[tauri::command(async)]
pub fn voice_preview(v: tauri::State<'_, Arc<VoiceService>>, voice: String) -> Result<u32, String> {
    v.preview(&voice)
}

/// VB-CABLE's download page (fixed address), in the browser.
pub const CABLE_PAGE: &str = "https://vb-audio.com/Cable/";

#[tauri::command(async)]
pub fn voice_open_cable_page() -> Result<(), String> {
    crate::logs::shell_open(CABLE_PAGE)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn service(dir: &std::path::Path) -> Arc<VoiceService> {
        struct Quiet;
        impl DownloadEvents for Quiet {
            fn progress(&self, _: &DownloadStatus) {}
            fn finished(&self) {}
        }
        VoiceService::new(
            dir.to_path_buf(),
            Downloader::new(Arc::new(crate::downloads::HttpFetcher), Arc::new(Quiet)),
        )
    }

    #[test]
    fn off_by_default_with_a_voice_for_each_language_and_a_test_sentence() {
        let d = std::env::temp_dir().join(format!("crosstalk-voice-svc-{}", std::process::id()));
        let s = service(&d);
        assert!(!s.options().enabled);
        assert_eq!(s.voice_for("ru").unwrap().id, "ru_RU-irina-medium");
        let mut o = s.options();
        o.voices.insert("ru".into(), "ru_RU-dmitri-medium".into());
        // A voice of another language is ignored.
        o.voices.insert("en".into(), "ru_RU-dmitri-medium".into());
        *s.options.lock().unwrap() = o;
        assert_eq!(s.voice_for("ru").unwrap().id, "ru_RU-dmitri-medium");
        assert_eq!(s.voice_for("en").unwrap().id, "en_US-amy-medium");
        for lang in ["ru", "en", "fr", "de", "es", "pl", "tr", "uk"] {
            assert_ne!(sample_sentence(lang), "Two on B.", "{lang}");
        }
    }

    #[test]
    fn saying_needs_the_voice_then_the_cable() {
        let d = std::env::temp_dir().join(format!("crosstalk-voice-say-{}", std::process::id()));
        let s = service(&d);
        assert!(s
            .say("Двое на B", "ru")
            .unwrap_err()
            .contains("non téléchargée"));
        assert!(s.say("x", "xx").unwrap_err().contains("aucune voix"));
        // Turned on without VB-CABLE: a clear error, nothing routed.
        s.configure(VoiceOptions {
            enabled: true,
            ..Default::default()
        });
        let st = s.status();
        assert!(!st.routing);
        assert!(st.error.is_some());
        assert_eq!(st.voices.len(), catalog::VOICES.len());
        s.configure(VoiceOptions::default());
        assert!(s.status().error.is_none());
    }

    /// The router thread as on Windows: blocks of 10 ms, a 20 ms device buffer.
    fn fake_router(
        feed: Arc<router::Feed>,
        stop: Arc<std::sync::atomic::AtomicBool>,
    ) -> std::thread::JoinHandle<()> {
        std::thread::spawn(move || {
            let mut device = 0u64;
            while !stop.load(std::sync::atomic::Ordering::SeqCst) {
                let room = 960u64.saturating_sub(device) as usize;
                feed.take_voice(room);
                device += room as u64;
                feed.wrote(room, device as u32);
                std::thread::sleep(std::time::Duration::from_millis(10));
                device = device.saturating_sub(480);
            }
        })
    }

    #[test]
    fn consecutive_sentences_are_serialised_one_key_press_each() {
        use ptt::tests::{Ev, FakeKeys};
        let d = std::env::temp_dir().join(format!("crosstalk-voice-ptt-{}", std::process::id()));
        let keys = Arc::new(FakeKeys::default());
        let s = VoiceService::with_keys(
            d,
            Downloader::new(Arc::new(crate::downloads::HttpFetcher), Arc::new(QuietDl)),
            keys.clone(),
        );
        let phases = Arc::new(Mutex::new(Vec::new()));
        let p2 = phases.clone();
        s.on_ptt(move |st| p2.lock().unwrap().push(st.phase));
        let feed = Arc::new(router::Feed::new(48_000));
        let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let r = fake_router(feed.clone(), stop.clone());
        let plan = ptt::Plan::new(true, "K", 60, 100).unwrap();
        for _ in 0..3 {
            s.enqueue(Job {
                samples: vec![0.1; 4_800],
                rate: 48_000,
                plan,
                feed: feed.clone(),
                generation: s.ptt.generation.current(),
            })
            .unwrap();
        }
        let started = std::time::Instant::now();
        while phases
            .lock()
            .unwrap()
            .iter()
            .filter(|p| **p == ptt::Phase::Done)
            .count()
            < 3
        {
            assert!(started.elapsed() < std::time::Duration::from_secs(10));
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        stop.store(true, std::sync::atomic::Ordering::SeqCst);
        r.join().unwrap();
        // Down, play, up — three times, never overlapping.
        let k = ptt::PttKey::Key(0x4B);
        let ev: Vec<Ev> = keys.events();
        assert_eq!(
            ev,
            [
                Ev::Down(k),
                Ev::Up(k),
                Ev::Down(k),
                Ev::Up(k),
                Ev::Down(k),
                Ev::Up(k)
            ]
        );
        assert!(s.ptt.held.lock().unwrap().is_none());
        assert!(!s.holding_key());
        // While a key is held, the Mouse 4 / Mouse 5 taps are ignored (tap_keys.rs).
        *s.ptt.held.lock().unwrap() = Some(k);
        assert!(s.holding_key());
        *s.ptt.held.lock().unwrap() = None;
    }

    #[test]
    fn cancel_drops_the_queued_sentences_and_releases_the_key() {
        use ptt::tests::{Ev, FakeKeys};
        let d = std::env::temp_dir().join(format!("crosstalk-voice-cancel-{}", std::process::id()));
        let keys = Arc::new(FakeKeys::default());
        let s = VoiceService::with_keys(
            d,
            Downloader::new(Arc::new(crate::downloads::HttpFetcher), Arc::new(QuietDl)),
            keys.clone(),
        );
        let phases = Arc::new(Mutex::new(Vec::new()));
        let p2 = phases.clone();
        s.on_ptt(move |st| p2.lock().unwrap().push(st.phase));
        let feed = Arc::new(router::Feed::new(48_000));
        let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let r = fake_router(feed.clone(), stop.clone());
        let plan = ptt::Plan::new(true, "K", 60, 100).unwrap();
        for _ in 0..2 {
            s.enqueue(Job {
                samples: vec![0.1; 96_000],
                rate: 48_000,
                plan,
                feed: feed.clone(),
                generation: s.ptt.generation.current(),
            })
            .unwrap();
        }
        std::thread::sleep(std::time::Duration::from_millis(300));
        s.cancel();
        let started = std::time::Instant::now();
        while phases
            .lock()
            .unwrap()
            .iter()
            .filter(|p| **p == ptt::Phase::Cancelled)
            .count()
            < 2
        {
            assert!(started.elapsed() < std::time::Duration::from_secs(5));
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        stop.store(true, std::sync::atomic::Ordering::SeqCst);
        r.join().unwrap();
        let k = ptt::PttKey::Key(0x4B);
        assert_eq!(keys.events(), [Ev::Down(k), Ev::Up(k)]);
        assert!(feed.voice.is_empty());
        assert_eq!(s.ptt_status().phase, ptt::Phase::Cancelled);
    }

    #[test]
    fn a_wrong_key_is_reported_before_any_synthesis() {
        let d = std::env::temp_dir().join(format!("crosstalk-voice-key-{}", std::process::id()));
        let s = service(&d);
        *s.options.lock().unwrap() = VoiceOptions {
            enabled: true,
            auto_ptt: true,
            ptt_key: "Échap".into(),
            ..Default::default()
        };
        assert!(s
            .say("Двое на B", "ru")
            .unwrap_err()
            .contains("touche inconnue"));
        let st = s.ptt_status();
        assert_eq!(st.phase, ptt::Phase::Error);
        assert!(st.detail.unwrap().contains("touche inconnue"));
    }

    struct QuietDl;
    impl DownloadEvents for QuietDl {
        fn progress(&self, _: &DownloadStatus) {}
        fn finished(&self) {}
    }

    #[test]
    fn options_from_the_interface_fill_in_defaults() {
        let o: VoiceOptions =
            serde_json::from_str(r#"{"enabled":true,"voices":{"ru":"ru_RU-dmitri-medium"}}"#)
                .unwrap();
        assert!(o.enabled && o.monitor);
        assert_eq!((o.volume, o.mic_volume), (100, 100));
        // Automatic push-to-talk: off, K, default delays (settings of earlier versions).
        assert!(!o.auto_ptt);
        assert_eq!(o.ptt_key, "K");
        assert_eq!(
            (o.ptt_lead_ms, o.ptt_tail_ms),
            (ptt::DEFAULT_LEAD_MS, ptt::DEFAULT_TAIL_MS)
        );
        assert_eq!(o.plan().unwrap().key, None);
    }
}
