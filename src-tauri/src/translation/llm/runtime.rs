//! One TranslateGemma helper shared by every language pair: started when the
//! translation engine loads, kept for the session, stopped when translation
//! is turned off or Opus-MT is chosen. Measures its latency and steps aside
//! (automatic mode) when it is too slow on this PC or keeps failing.
//!
//! Locks: the process and the statistics are behind separate mutexes, so the
//! UI reading the status never waits for a generation in progress.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::{Deserialize, Serialize};

use super::catalog::{self, Install, LlmSpec};
use super::process::{Launch, LlmProcess, Ready};
use super::prompt;

/// Which engine translates the lines the lexicon does not know.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum EngineChoice {
    /// TranslateGemma when installed and fast enough, otherwise Opus-MT.
    #[default]
    Auto,
    /// TranslateGemma even when slow (Opus-MT only if it fails).
    Translategemma,
    /// Opus-MT only (TranslateGemma not loaded).
    Opus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Backend {
    /// Graphics card when it has enough free memory next to CS2, else CPU.
    #[default]
    Auto,
    Cpu,
    Gpu,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum LlmState {
    /// Model not downloaded (or helper missing).
    Missing,
    /// Model present but its fingerprint was never checked.
    Unverified,
    /// Opus-MT chosen or translation off: nothing loaded.
    Off,
    /// Installed, not started yet.
    Idle,
    Loading,
    Ready,
    /// Works, but too slow for live callouts on this PC.
    Slow,
    Error,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LlmStatus {
    pub id: &'static str,
    pub name: &'static str,
    pub quant: &'static str,
    pub repo: &'static str,
    pub file: &'static str,
    pub license: &'static str,
    pub license_url: &'static str,
    pub size_mb: u32,
    pub ram_mb: u32,
    pub install: Install,
    pub helper: bool,
    pub state: LlmState,
    pub choice: EngineChoice,
    pub backend_choice: Backend,
    /// "cpu" or "vulkan" once loaded.
    pub backend: Option<String>,
    pub device: Option<String>,
    pub gpu_skipped: Option<String>,
    pub threads: Option<i32>,
    pub load_ms: Option<u32>,
    pub last_ms: Option<u32>,
    pub p50_ms: Option<u32>,
    pub p95_ms: Option<u32>,
    pub translations: u32,
    pub rss_mb: Option<u32>,
    pub error: Option<String>,
}

/// A line slower than this (median of the last ones) is late for a callout.
pub const SLOW_MS: u32 = 3_500;
/// Hard limit for one line: past it the helper is restarted.
pub const LINE_TIMEOUT: Duration = Duration::from_secs(12);
pub const LOAD_TIMEOUT: Duration = Duration::from_secs(180);
/// Consecutive failed starts before giving up for the session.
pub const MAX_FAILED_STARTS: u32 = 3;
const WINDOW: usize = 32;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Settings {
    pub choice: EngineChoice,
    pub backend: Backend,
    /// Generation threads; 0: automatic.
    pub threads: u32,
    /// Translation turned on in the settings.
    pub enabled: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            choice: EngineChoice::Auto,
            backend: Backend::Auto,
            threads: 0,
            enabled: true,
        }
    }
}

/// How the helper is started; a trait so the service is tested without it.
pub trait Spawner: Send + Sync {
    fn spawn(&self, launch: &Launch) -> Result<LlmProcess, String>;
}

pub struct RealSpawner;

impl Spawner for RealSpawner {
    fn spawn(&self, launch: &Launch) -> Result<LlmProcess, String> {
        LlmProcess::spawn(launch, LOAD_TIMEOUT)
    }
}

#[derive(Default)]
struct Stats {
    state_override: Option<LlmState>,
    ready: Option<Ready>,
    latencies: VecDeque<u32>,
    last_ms: Option<u32>,
    translations: u32,
    rss_mb: Option<u32>,
    error: Option<String>,
    failed_starts: u32,
    slow: bool,
}

pub struct LlmRuntime {
    models_dir: PathBuf,
    spec: &'static LlmSpec,
    helper: PathBuf,
    spawner: Box<dyn Spawner>,
    settings: Mutex<Settings>,
    process: Mutex<Option<LlmProcess>>,
    stats: Mutex<Stats>,
}

/// Threads for generation: half the logical cores (about the physical ones),
/// 2 to 6, leaving the rest to CS2 and Whisper.
pub fn auto_threads(logical: usize) -> u32 {
    (logical / 2).clamp(2, 6) as u32
}

pub fn percentile(sorted: &[u32], p: f64) -> Option<u32> {
    if sorted.is_empty() {
        return None;
    }
    let i = ((sorted.len() - 1) as f64 * p).round() as usize;
    Some(sorted[i])
}

impl LlmRuntime {
    pub fn new(models_dir: PathBuf, helper: PathBuf, spawner: Box<dyn Spawner>) -> Arc<Self> {
        Self::with_spec(models_dir, helper, spawner, catalog::default_model())
    }

    pub fn with_spec(
        models_dir: PathBuf,
        helper: PathBuf,
        spawner: Box<dyn Spawner>,
        spec: &'static LlmSpec,
    ) -> Arc<Self> {
        Arc::new(Self {
            models_dir,
            spec,
            helper,
            spawner,
            settings: Mutex::new(Settings::default()),
            process: Mutex::new(None),
            stats: Mutex::new(Stats::default()),
        })
    }

    pub fn spec(&self) -> &'static LlmSpec {
        self.spec
    }

    pub fn models_dir(&self) -> &Path {
        &self.models_dir
    }

    /// New settings; returns true when the helper had to be stopped for them.
    pub fn configure(&self, s: Settings) -> bool {
        let old = std::mem::replace(&mut *self.settings.lock().unwrap(), s);
        if old == s {
            return false;
        }
        let relaunch =
            old.backend != s.backend || old.threads != s.threads || !self.wanted_with(&s);
        {
            let mut st = self.stats.lock().unwrap();
            st.failed_starts = 0;
            st.slow = false;
            st.error = None;
            st.latencies.clear();
        }
        if relaunch {
            self.stop();
        }
        relaunch
    }

    pub fn settings(&self) -> Settings {
        *self.settings.lock().unwrap()
    }

    /// After a download, a repair or "Rechercher les modèles": try again.
    pub fn reset_failures(&self) {
        let mut st = self.stats.lock().unwrap();
        st.failed_starts = 0;
        st.slow = false;
        st.error = None;
    }

    pub fn install(&self) -> Install {
        catalog::install_state(&self.models_dir, self.spec)
    }

    fn wanted_with(&self, s: &Settings) -> bool {
        s.enabled && s.choice != EngineChoice::Opus
    }

    /// The model is downloaded, checked, and the helper is there.
    pub fn available(&self) -> bool {
        matches!(self.install(), Install::Verified { .. }) && self.helper.is_file()
    }

    /// Lines should go to TranslateGemma now.
    pub fn usable(&self) -> bool {
        let s = self.settings();
        if !self.wanted_with(&s) || !self.available() {
            return false;
        }
        let st = self.stats.lock().unwrap();
        if st.failed_starts >= MAX_FAILED_STARTS {
            return false;
        }
        !(st.slow && s.choice == EngineChoice::Auto)
    }

    fn launch(&self) -> Launch {
        let s = self.settings();
        let logical = std::thread::available_parallelism().map_or(4, |n| n.get());
        let size_mb = self.spec.bytes / (1024 * 1024);
        Launch {
            exe: self.helper.clone(),
            model: catalog::model_path(&self.models_dir, self.spec),
            threads: if s.threads > 0 {
                s.threads
            } else {
                auto_threads(logical)
            },
            gpu_layers: if s.backend == Backend::Cpu { 0 } else { 99 },
            // Model + context + room for CS2 and Whisper on the card.
            gpu_min_free_mb: if s.backend == Backend::Auto {
                size_mb + 1_536
            } else {
                0
            },
            ctx: self.spec.ctx,
        }
    }

    /// Starts the helper if needed (worker thread: may take seconds).
    pub fn ensure_started(&self) -> Result<(), String> {
        if !self.usable() {
            return Err(self
                .stats
                .lock()
                .unwrap()
                .error
                .clone()
                .unwrap_or_else(|| "TranslateGemma indisponible".into()));
        }
        let mut proc = self.process.lock().unwrap();
        if let Some(p) = proc.as_mut() {
            if !p.dead() {
                return Ok(());
            }
            *proc = None;
        }
        self.stats.lock().unwrap().state_override = Some(LlmState::Loading);
        match self.spawner.spawn(&self.launch()) {
            Ok(p) => {
                let mut st = self.stats.lock().unwrap();
                st.state_override = None;
                st.rss_mb = Some(p.ready.rss_mb).filter(|&m| m > 0);
                st.ready = Some(p.ready.clone());
                st.error = None;
                st.failed_starts = 0;
                *proc = Some(p);
                Ok(())
            }
            Err(e) => {
                let mut st = self.stats.lock().unwrap();
                st.state_override = None;
                st.failed_starts += 1;
                st.error = Some(e.clone());
                Err(e)
            }
        }
    }

    /// One line, `source` → `target`.
    pub fn translate(&self, source: &str, target: &str, text: &str) -> Result<String, String> {
        let p = prompt::translategemma(source, target, text)?;
        self.ensure_started()?;
        let mut proc = self.process.lock().unwrap();
        let Some(engine) = proc.as_mut() else {
            return Err("moteur arrêté".into());
        };
        let started = std::time::Instant::now();
        let reply = engine.request(&p, prompt::max_tokens(text), LINE_TIMEOUT);
        let ms = started.elapsed().as_millis() as u32;
        let dead = engine.dead();
        if dead {
            *proc = None;
        }
        drop(proc);
        let mut st = self.stats.lock().unwrap();
        match reply {
            Ok(r) => {
                st.latencies.push_back(ms);
                if st.latencies.len() > WINDOW {
                    st.latencies.pop_front();
                }
                st.last_ms = Some(ms);
                st.translations += 1;
                if r.rss_mb > 0 {
                    st.rss_mb = Some(r.rss_mb);
                }
                // Judged on the last few lines once there are enough of them.
                let recent: Vec<u32> = st.latencies.iter().rev().take(6).copied().collect();
                if recent.len() >= 4 {
                    let mut s = recent.clone();
                    s.sort_unstable();
                    st.slow = percentile(&s, 0.5).is_some_and(|m| m > SLOW_MS);
                }
                let out = prompt::clean_output(text, r.text.as_deref().unwrap_or_default());
                if out.is_empty() {
                    return Err("traduction vide".into());
                }
                Ok(out)
            }
            Err(e) => {
                if dead {
                    st.error = Some(e.clone());
                }
                Err(e)
            }
        }
    }

    pub fn stop(&self) {
        if let Some(mut p) = self.process.lock().unwrap().take() {
            p.stop();
        }
        let mut st = self.stats.lock().unwrap();
        st.ready = None;
        st.rss_mb = None;
    }

    pub fn running(&self) -> bool {
        self.process.try_lock().map(|p| p.is_some()).unwrap_or(true)
    }

    pub fn status(&self) -> LlmStatus {
        let s = self.settings();
        let install = self.install();
        let helper = self.helper.is_file();
        let st = self.stats.lock().unwrap();
        let mut sorted: Vec<u32> = st.latencies.iter().copied().collect();
        sorted.sort_unstable();
        let state = if let Some(o) = st.state_override {
            o
        } else if !helper
            || matches!(
                install,
                Install::Missing | Install::Partial { .. } | Install::Corrupt
            )
        {
            LlmState::Missing
        } else if install == Install::Unverified {
            LlmState::Unverified
        } else if !self.wanted_with(&s) {
            LlmState::Off
        } else if st.failed_starts >= MAX_FAILED_STARTS {
            LlmState::Error
        } else if st.ready.is_none() {
            if st.error.is_some() {
                LlmState::Error
            } else {
                LlmState::Idle
            }
        } else if st.slow {
            LlmState::Slow
        } else {
            LlmState::Ready
        };
        let r = st.ready.as_ref();
        LlmStatus {
            id: self.spec.id,
            name: self.spec.name,
            quant: self.spec.quant,
            repo: self.spec.repo,
            file: self.spec.file,
            license: self.spec.license,
            license_url: self.spec.license_url,
            size_mb: (self.spec.bytes / 1_000_000) as u32,
            ram_mb: self.spec.ram_mb,
            install,
            helper,
            state,
            choice: s.choice,
            backend_choice: s.backend,
            backend: r.map(|r| r.backend.clone()),
            device: r.and_then(|r| r.device.clone()),
            gpu_skipped: r.and_then(|r| r.gpu_skipped.clone()),
            threads: r.map(|r| r.threads),
            load_ms: r.map(|r| r.load_ms),
            last_ms: st.last_ms,
            p50_ms: percentile(&sorted, 0.5),
            p95_ms: percentile(&sorted, 0.95),
            translations: st.translations,
            rss_mb: st.rss_mb,
            error: st.error.clone(),
        }
    }
}

impl Drop for LlmRuntime {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
pub mod tests {
    use super::super::process::tests::{ready, Sink};
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::mpsc;

    /// A helper that answers every request with `reply(prompt)` after `delay_ms`.
    pub struct FakeSpawner {
        pub spawns: Arc<AtomicU32>,
        pub fail: bool,
        pub delay_ms: u32,
        pub launches: Arc<Mutex<Vec<Launch>>>,
    }

    impl Default for FakeSpawner {
        fn default() -> Self {
            Self::new()
        }
    }

    impl FakeSpawner {
        pub fn new() -> Self {
            Self {
                spawns: Arc::new(AtomicU32::new(0)),
                fail: false,
                delay_ms: 0,
                launches: Arc::new(Mutex::new(Vec::new())),
            }
        }
    }

    /// Echo "FR(<text>)" from the prompt's last line before <end_of_turn>.
    struct Echo {
        sink: Sink,
        tx: mpsc::Sender<String>,
        seen: usize,
        delay_ms: u32,
    }
    impl std::io::Write for Echo {
        fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
            self.sink.write(b)?;
            let all = String::from_utf8(self.sink.0.lock().unwrap().clone()).unwrap();
            for line in all.lines().skip(self.seen) {
                if !all.ends_with('\n') {
                    break;
                }
                self.seen += 1;
                let v: serde_json::Value = serde_json::from_str(line).unwrap();
                let p = v["prompt"].as_str().unwrap();
                let text = p
                    .split("\n\n\n")
                    .nth(1)
                    .unwrap()
                    .split("<end_of_turn>")
                    .next()
                    .unwrap();
                let reply = serde_json::json!({"id": v["id"], "text": format!("FR({text})"), "ms": self.delay_ms});
                std::thread::sleep(Duration::from_millis(self.delay_ms as u64));
                let _ = self.tx.send(reply.to_string());
            }
            Ok(b.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    impl Spawner for FakeSpawner {
        fn spawn(&self, launch: &Launch) -> Result<LlmProcess, String> {
            self.spawns.fetch_add(1, Ordering::SeqCst);
            self.launches.lock().unwrap().push(launch.clone());
            if self.fail {
                return Err("chargement du modèle impossible".into());
            }
            let (tx, rx) = mpsc::channel();
            let echo = Echo {
                sink: Sink::default(),
                tx,
                seen: 0,
                delay_ms: self.delay_ms,
            };
            Ok(LlmProcess::from_parts(Box::new(echo), rx, ready()))
        }
    }

    /// A models dir where the model "is installed" (sparse file + manifest).
    pub fn installed_dir(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("crosstalk-llmrt-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let m = catalog::default_model();
        std::fs::create_dir_all(dir.join(m.dir)).unwrap();
        std::fs::File::create(catalog::model_path(&dir, m))
            .unwrap()
            .set_len(m.bytes)
            .unwrap();
        catalog::write_manifest(
            &catalog::manifest_path(&dir, m),
            &catalog::Manifest {
                model: m.id.into(),
                file: m.file.into(),
                bytes: m.bytes,
                sha256: "ab".repeat(32),
                source: catalog::url(m),
                verified_at: 0,
            },
        )
        .unwrap();
        std::fs::write(dir.join("crosstalk-llm"), b"").unwrap();
        dir
    }

    pub fn runtime(dir: &Path, spawner: FakeSpawner) -> Arc<LlmRuntime> {
        LlmRuntime::new(dir.into(), dir.join("crosstalk-llm"), Box::new(spawner))
    }

    #[test]
    fn starts_once_and_translates_with_the_official_prompt() {
        let dir = installed_dir("once");
        let sp = FakeSpawner::new();
        let spawns = sp.spawns.clone();
        let rt = runtime(&dir, sp);
        assert_eq!(rt.status().state, LlmState::Idle);
        assert_eq!(
            rt.translate("ru", "fr", "два на Б").unwrap(),
            "FR(два на Б)"
        );
        assert_eq!(rt.translate("en", "fr", "rush B").unwrap(), "FR(rush B)");
        assert_eq!(spawns.load(Ordering::SeqCst), 1);
        let st = rt.status();
        assert_eq!(st.state, LlmState::Ready);
        assert_eq!(st.translations, 2);
        assert_eq!(st.backend.as_deref(), Some("cpu"));
        assert!(st.p50_ms.is_some());
        rt.stop();
        assert!(!rt.running());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn missing_or_unverified_models_are_not_used() {
        let dir = installed_dir("missing");
        let rt = runtime(&dir, FakeSpawner::new());
        std::fs::remove_file(catalog::manifest_path(&dir, rt.spec())).unwrap();
        assert_eq!(rt.status().state, LlmState::Unverified);
        assert!(!rt.usable());
        std::fs::remove_file(catalog::model_path(&dir, rt.spec())).unwrap();
        assert_eq!(rt.status().state, LlmState::Missing);
        assert!(rt.translate("ru", "fr", "да").is_err());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn choosing_opus_or_turning_translation_off_stops_the_engine() {
        let dir = installed_dir("opus");
        let rt = runtime(&dir, FakeSpawner::new());
        rt.translate("ru", "fr", "да").unwrap();
        assert!(rt.running());
        assert!(rt.configure(Settings {
            choice: EngineChoice::Opus,
            ..Settings::default()
        }));
        assert!(!rt.running());
        assert_eq!(rt.status().state, LlmState::Off);
        assert!(!rt.usable());
        rt.configure(Settings::default());
        rt.translate("ru", "fr", "да").unwrap();
        rt.configure(Settings {
            enabled: false,
            ..Settings::default()
        });
        assert!(!rt.running());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn repeated_start_failures_give_up_until_reset() {
        let dir = installed_dir("fail");
        let mut sp = FakeSpawner::new();
        sp.fail = true;
        let spawns = sp.spawns.clone();
        let rt = runtime(&dir, sp);
        for _ in 0..5 {
            assert!(rt.translate("ru", "fr", "да").is_err());
        }
        assert_eq!(spawns.load(Ordering::SeqCst), MAX_FAILED_STARTS);
        let st = rt.status();
        assert_eq!(st.state, LlmState::Error);
        assert!(st.error.unwrap().contains("impossible"));
        assert!(!rt.usable());
        rt.reset_failures();
        assert!(rt.usable());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_slow_engine_steps_aside_in_automatic_mode_only() {
        let dir = installed_dir("slow");
        let rt = runtime(&dir, FakeSpawner::new());
        rt.stats
            .lock()
            .unwrap()
            .latencies
            .extend([5000, 5000, 5000, 5000, 5000]);
        // The median of the last six lines is still 5 s: judged slow.
        rt.translate("ru", "fr", "да").unwrap();
        assert_eq!(rt.status().state, LlmState::Slow);
        assert!(!rt.usable());
        rt.configure(Settings {
            choice: EngineChoice::Translategemma,
            ..Settings::default()
        });
        // Forced mode keeps using it even when slow.
        rt.stats.lock().unwrap().slow = true;
        assert!(rt.usable());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn launch_follows_the_backend_choice() {
        let dir = installed_dir("launch");
        let sp = FakeSpawner::new();
        let launches = sp.launches.clone();
        let rt = runtime(&dir, sp);
        rt.translate("ru", "fr", "да").unwrap();
        rt.configure(Settings {
            backend: Backend::Cpu,
            threads: 3,
            ..Settings::default()
        });
        rt.translate("ru", "fr", "да").unwrap();
        let l = launches.lock().unwrap();
        assert_eq!(l[0].gpu_layers, 99);
        assert!(l[0].gpu_min_free_mb > 2_000);
        assert_eq!((l[1].gpu_layers, l[1].threads), (0, 3));
        drop(l);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn thread_count_and_percentiles() {
        assert_eq!(auto_threads(4), 2);
        assert_eq!(auto_threads(12), 6);
        assert_eq!(auto_threads(32), 6);
        assert_eq!(percentile(&[], 0.5), None);
        assert_eq!(percentile(&[1, 2, 3, 4, 100], 0.5), Some(3));
        assert_eq!(percentile(&[1, 2, 3, 4, 100], 0.95), Some(100));
    }
}
