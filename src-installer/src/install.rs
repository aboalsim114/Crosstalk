//! The installation itself, in the four phases the UI shows:
//!
//! 0. PRÉPARATION   (0–12 %)  free space, Crosstalk not running.
//! 1. EXTRACTION    (12–62 %) application files into a staging folder, then
//!    the models (Silero VAD, Whisper base/small, TranslateGemma 4B, Opus-MT
//!    as fallback) downloaded into %LOCALAPPDATA%\Crosstalk\models, weighted
//!    by their size; TranslateGemma is checked by SHA-256.
//! 2. CONFIGURATION (62–88 %) staged files moved into place, Crosstalk
//!    registered for the user (HKCU\Software\Crosstalk).
//! 3. FINALISATION  (88–100 %) Start menu (and desktop) shortcuts, then
//!    TranslateGemma loaded once by the installed engine (`crosstalk-llm
//!    --check`): a failure is reported, not fatal (Opus-MT takes over).
//!
//! Cancelling during phases 0–1 removes the staging folder: an installed
//! version stays intact and downloaded models are kept (the next attempt
//! resumes them). Windows specifics go through [`Platform`] so the sequence is
//! tested on any OS.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};

use serde::Serialize;

use crate::downloads::{self, DownloadEvents, DownloadState, DownloadStatus, Downloader, Fetcher};
use crate::translation::llm::catalog as llm;

/// One embedded application file, deflated (see build.rs).
pub struct Entry {
    pub name: &'static str,
    pub size: u64,
    pub data: &'static [u8],
}

pub const APP_EXE: &str = "crosstalk.exe";
pub const LLM_EXE: &str = "crosstalk-llm.exe";
/// Whisper models installed: every one the app runs on the CPU in this
/// version (base: fast, small: more precise). Medium and large-v3-turbo, too
/// slow without a GPU, can still be fetched from the app.
pub const WHISPER_MODELS: &[&str] = &["base", "small"];
const STAGING: &str = ".crosstalk-setup";
const MB: f64 = 1e6;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    pub phase: u8,
    pub pct: f64,
    pub detail: String,
    pub done: bool,
    pub error: Option<String>,
    pub cancelled: bool,
    /// Installed, but something to know (TranslateGemma could not load).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warning: Option<String>,
}

impl Progress {
    fn at(phase: u8, pct: f64, detail: impl Into<String>) -> Self {
        Self {
            phase,
            pct: pct.clamp(0.0, 100.0),
            detail: detail.into(),
            done: false,
            error: None,
            cancelled: false,
            warning: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Options {
    pub dir: PathBuf,
    pub desktop: bool,
    /// Download the models now (off only for offline tests: the app then
    /// fetches them at first launch).
    pub models: bool,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Stop {
    Cancelled,
    Failed(String),
}

pub trait Platform: Send + Sync {
    /// Free bytes on the volume holding `dir` (or its nearest existing parent).
    fn free_space(&self, dir: &Path) -> Option<u64>;
    /// Same volume (models and application share the free space).
    fn same_volume(&self, a: &Path, b: &Path) -> bool;
    /// Records the installation for the user (folder, version).
    fn register(&self, dir: &Path, version: &str) -> Result<(), String>;
    /// Start menu shortcut, and the desktop one when asked (removed otherwise).
    fn shortcuts(&self, exe: &Path, desktop: bool) -> Result<(), String>;
    /// Loads the translation model once with the installed engine, without
    /// a window; returns where it runs ("cpu", "vulkan").
    fn check_llm(&self, exe: &Path, model: &Path) -> Result<String, String> {
        check_llm(exe, model, std::time::Duration::from_secs(240))
    }
}

/// `crosstalk-llm --check`: the ready line, or why the model did not load.
pub fn check_llm(exe: &Path, model: &Path, timeout: std::time::Duration) -> Result<String, String> {
    use std::io::BufRead;
    let mut cmd = std::process::Command::new(exe);
    cmd.arg("--model")
        .arg(model)
        .arg("--check")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    let mut child = cmd.spawn().map_err(|e| format!("moteur introuvable : {e}"))?;
    let out = child.stdout.take().expect("piped");
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let mut line = String::new();
        let _ = std::io::BufReader::new(out).read_line(&mut line);
        let _ = tx.send(line);
    });
    let line = match rx.recv_timeout(timeout) {
        Ok(l) => l,
        Err(_) => {
            let _ = child.kill();
            let _ = child.wait();
            return Err("chargement trop long".into());
        }
    };
    let _ = child.wait();
    let v: serde_json::Value =
        serde_json::from_str(line.trim()).map_err(|_| "réponse illisible du moteur".to_string())?;
    match v["event"].as_str() {
        Some("ready") => Ok(v["backend"].as_str().unwrap_or("cpu").to_string()),
        _ => Err(v["message"].as_str().unwrap_or("erreur inconnue").to_string()),
    }
}

pub struct Installer {
    pub files: &'static [Entry],
    pub models_dir: PathBuf,
    pub version: &'static str,
    fetcher: Arc<dyn Fetcher>,
    platform: Arc<dyn Platform>,
    cancel: AtomicBool,
    running: AtomicBool,
    installed: AtomicBool,
    downloader: Mutex<Option<Arc<Downloader>>>,
    warning: Mutex<Option<String>>,
}

/// Download progress, forwarded to the installation thread.
struct Forward(Mutex<mpsc::Sender<DownloadStatus>>);

impl DownloadEvents for Forward {
    fn progress(&self, status: &DownloadStatus) {
        let _ = self.0.lock().unwrap().send(status.clone());
    }
    fn finished(&self) {}
}

fn mb(bytes: u64) -> String {
    format!("{:.0}", bytes as f64 / MB)
}

impl Installer {
    pub fn new(
        files: &'static [Entry],
        models_dir: PathBuf,
        version: &'static str,
        fetcher: Arc<dyn Fetcher>,
        platform: Arc<dyn Platform>,
    ) -> Arc<Self> {
        Arc::new(Self {
            files,
            models_dir,
            version,
            fetcher,
            platform,
            cancel: AtomicBool::new(false),
            running: AtomicBool::new(false),
            installed: AtomicBool::new(false),
            downloader: Mutex::new(None),
            warning: Mutex::new(None),
        })
    }

    pub fn app_bytes(&self) -> u64 {
        self.files.iter().map(|f| f.size).sum()
    }

    /// Models still to download (0 when a previous install already has them).
    pub fn model_bytes(&self) -> u64 {
        self.models_todo().iter().map(|f| f.bytes).sum()
    }

    fn models_todo(&self) -> Vec<downloads::FileSpec> {
        downloads::missing_all(&self.models_dir, WHISPER_MODELS, true)
    }

    pub fn platform(&self) -> &dyn Platform {
        &*self.platform
    }

    /// True once an installation completed in this session.
    pub fn installed(&self) -> bool {
        self.installed.load(Ordering::SeqCst)
    }

    /// Stops the installation if it is still in phase 0 or 1.
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::SeqCst);
        if let Some(d) = self.downloader.lock().unwrap().as_ref() {
            d.cancel();
        }
    }

    fn check(&self) -> Result<(), Stop> {
        if self.cancel.load(Ordering::SeqCst) {
            Err(Stop::Cancelled)
        } else {
            Ok(())
        }
    }

    /// Runs the whole installation on the calling thread; `report` receives
    /// every step, including the final one (done, error or cancelled).
    pub fn run(&self, opts: &Options, report: &dyn Fn(Progress)) -> Result<(), Stop> {
        if self.running.swap(true, Ordering::SeqCst) {
            return Err(Stop::Failed("une installation est déjà en cours".into()));
        }
        self.cancel.store(false, Ordering::SeqCst);
        *self.warning.lock().unwrap() = None;
        let staging = opts.dir.join(STAGING);
        let result = self.steps(opts, &staging, report);
        *self.downloader.lock().unwrap() = None;
        let last = match &result {
            Ok(()) => {
                self.installed.store(true, Ordering::SeqCst);
                Progress {
                    done: true,
                    warning: self.warning.lock().unwrap().clone(),
                    ..Progress::at(3, 100.0, "Installation terminée")
                }
            }
            Err(stop) => {
                let _ = std::fs::remove_dir_all(&staging);
                match stop {
                    Stop::Cancelled => Progress {
                        cancelled: true,
                        ..Progress::at(0, 0.0, "Installation annulée")
                    },
                    Stop::Failed(e) => Progress {
                        error: Some(e.clone()),
                        ..Progress::at(0, 0.0, "")
                    },
                }
            }
        };
        report(last);
        self.running.store(false, Ordering::SeqCst);
        result
    }

    fn steps(&self, opts: &Options, staging: &Path, report: &dyn Fn(Progress)) -> Result<(), Stop> {
        let fail = |e: String| Stop::Failed(e);
        let dir = &opts.dir;

        // 0. PRÉPARATION
        report(Progress::at(0, 2.0, "Vérification de l’espace disponible"));
        let app = self.app_bytes();
        let todo = if opts.models {
            self.models_todo()
        } else {
            Vec::new()
        };
        let models: u64 = todo.iter().map(|f| f.bytes).sum::<u64>()
            - todo.iter().map(downloads::partial_len).sum::<u64>();
        let free_dir = self.platform.free_space(dir);
        let need_dir = app
            + if self.platform.same_volume(dir, &self.models_dir) {
                models
            } else {
                0
            };
        if let Some(free) = free_dir {
            if free < need_dir {
                return Err(fail(format!(
                    "Espace insuffisant : {} Mo nécessaires, {} Mo disponibles.",
                    mb(need_dir),
                    mb(free)
                )));
            }
        }
        if !self.platform.same_volume(dir, &self.models_dir) {
            if let Some(free) = self.platform.free_space(&self.models_dir) {
                if free < models {
                    return Err(fail(format!(
                        "Espace insuffisant pour les modèles : {} Mo nécessaires, {} Mo disponibles.",
                        mb(models),
                        mb(free)
                    )));
                }
            }
        }
        ensure_not_running(dir).map_err(fail)?;
        std::fs::create_dir_all(dir)
            .map_err(|e| fail(format!("Impossible de créer {} : {e}", dir.display())))?;
        self.check()?;
        report(Progress::at(0, 12.0, "Espace disponible vérifié"));

        // 1. EXTRACTION, weighted by bytes: application then models.
        let total = (app + todo.iter().map(|f| f.bytes).sum::<u64>()).max(1) as f64;
        let pct = |done: u64| 12.0 + 50.0 * done as f64 / total;
        let _ = std::fs::remove_dir_all(staging);
        std::fs::create_dir_all(staging)
            .map_err(|e| fail(format!("Impossible d’écrire dans {} : {e}", dir.display())))?;
        let mut done = 0u64;
        for f in self.files {
            self.check()?;
            let mut src = flate2::read::DeflateDecoder::new(f.data);
            let path = staging.join(f.name);
            let mut out = std::fs::File::create(&path)
                .map_err(|e| fail(format!("Écriture de {} impossible : {e}", f.name)))?;
            let mut buf = vec![0u8; 256 * 1024];
            let mut written = 0u64;
            loop {
                let n = src
                    .read(&mut buf)
                    .map_err(|e| fail(format!("Fichier {} corrompu : {e}", f.name)))?;
                if n == 0 {
                    break;
                }
                out.write_all(&buf[..n])
                    .map_err(|e| fail(format!("Écriture de {} impossible : {e}", f.name)))?;
                written += n as u64;
                self.check()?;
                report(Progress::at(
                    1,
                    pct(done + written),
                    format!(
                        "Copie de l’application · {} / {} Mo",
                        mb(done + written),
                        mb(app)
                    ),
                ));
            }
            if written != f.size {
                return Err(fail(format!(
                    "Fichier {} incomplet ({written} octets)",
                    f.name
                )));
            }
            done += written;
        }
        if !todo.is_empty() {
            self.download_models(report, &|d| pct(app + d))?;
        }
        self.check()?;

        // 2. CONFIGURATION: from here on the installation is no longer cancelled.
        report(Progress::at(2, 64.0, "Mise en place des fichiers"));
        ensure_not_running(dir).map_err(fail)?;
        for (i, f) in self.files.iter().enumerate() {
            std::fs::rename(staging.join(f.name), dir.join(f.name))
                .map_err(|e| fail(format!("Installation de {} impossible : {e}", f.name)))?;
            report(Progress::at(
                2,
                64.0 + 16.0 * (i + 1) as f64 / self.files.len().max(1) as f64,
                "Mise en place des fichiers",
            ));
        }
        let _ = std::fs::remove_dir_all(staging);
        self.platform.register(dir, self.version).map_err(fail)?;
        report(Progress::at(2, 88.0, "Crosstalk enregistré dans Windows"));

        // 3. FINALISATION
        report(Progress::at(
            3,
            92.0,
            if opts.desktop {
                "Création des raccourcis (menu Démarrer, bureau)"
            } else {
                "Création du raccourci (menu Démarrer uniquement)"
            },
        ));
        self.platform
            .shortcuts(&dir.join(APP_EXE), opts.desktop)
            .map_err(fail)?;

        // The translation model must really load, not only be present.
        let m = llm::default_model();
        let engine = dir.join(LLM_EXE);
        let verified = matches!(
            llm::install_state(&self.models_dir, m),
            llm::Install::Verified { .. }
        );
        if verified && engine.is_file() {
            report(Progress::at(3, 95.0, format!("Chargement de test de {}", m.name)));
            match self
                .platform
                .check_llm(&engine, &llm::model_path(&self.models_dir, m))
            {
                Ok(backend) => report(Progress::at(
                    3,
                    99.0,
                    format!("{} prêt ({})", m.name, if backend == "cpu" { "processeur".to_string() } else { format!("carte graphique, {backend}") }),
                )),
                Err(e) => {
                    *self.warning.lock().unwrap() = Some(format!(
                        "{} n’a pas pu être chargé ({e}). Crosstalk utilisera Opus-MT ; Audio & IA › Traduction permet de réessayer.",
                        m.name
                    ));
                }
            }
        }
        Ok(())
    }

    fn download_models(
        &self,
        report: &dyn Fn(Progress),
        pct: &dyn Fn(u64) -> f64,
    ) -> Result<(), Stop> {
        let (tx, rx) = mpsc::channel();
        let d = Downloader::new(self.fetcher.clone(), Arc::new(Forward(Mutex::new(tx))));
        *self.downloader.lock().unwrap() = Some(d.clone());
        self.check()?;
        if !d.start_all(self.models_dir.clone(), WHISPER_MODELS, true) {
            return Ok(());
        }
        for st in rx {
            match st.state {
                DownloadState::Downloading | DownloadState::Idle => report(Progress::at(
                    1,
                    pct(st.done_bytes),
                    if st.verifying {
                        format!(
                            "{} · vérification de l’intégrité (SHA-256)",
                            st.current.as_deref().unwrap_or("Modèles")
                        )
                    } else {
                        format!(
                            "{} · {} / {} Mo · {:.0} %{}",
                            st.current.as_deref().unwrap_or("Modèles"),
                            mb(st.done_bytes),
                            mb(st.total_bytes),
                            100.0 * st.done_bytes as f64 / st.total_bytes.max(1) as f64,
                            if st.bytes_per_sec > 0 {
                                format!(" · {:.1} Mo/s", st.bytes_per_sec as f64 / MB)
                            } else {
                                String::new()
                            }
                        )
                    },
                )),
                DownloadState::Done => return Ok(()),
                DownloadState::Cancelled => return Err(Stop::Cancelled),
                DownloadState::Error => {
                    return Err(Stop::Failed(format!(
                        "Téléchargement des modèles interrompu : {}. Vérifiez la connexion puis réessayez (le téléchargement reprendra où il s’est arrêté).",
                        st.error.unwrap_or_default()
                    )))
                }
            }
        }
        Err(Stop::Failed("téléchargement des modèles interrompu".into()))
    }
}

/// In-app update: Crosstalk quits right after starting the installer; wait
/// (polling, without touching the process) until its exe can be replaced.
pub fn wait_until_closed(dir: &Path, timeout: std::time::Duration) -> bool {
    let end = std::time::Instant::now() + timeout;
    while ensure_not_running(dir).is_err() {
        if std::time::Instant::now() > end {
            return false;
        }
        std::thread::sleep(std::time::Duration::from_millis(250));
    }
    true
}

/// A running crosstalk.exe cannot be replaced: say so before copying anything.
fn ensure_not_running(dir: &Path) -> Result<(), String> {
    let exe = dir.join(APP_EXE);
    if !exe.exists() {
        return Ok(());
    }
    match std::fs::OpenOptions::new().append(true).open(&exe) {
        Ok(_) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied && cfg!(not(windows)) => Ok(()),
        Err(_) => Err("Crosstalk est ouvert : quittez-le (icône près de l’horloge › Quitter), puis réessayez.".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::speech::models as whisper;
    use std::sync::atomic::AtomicUsize;

    fn pack(data: &[u8]) -> &'static [u8] {
        let mut e = flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::fast());
        e.write_all(data).unwrap();
        Box::leak(e.finish().unwrap().into_boxed_slice())
    }

    fn files() -> &'static [Entry] {
        let exe: Vec<u8> = (0..700_000u32).map(|i| (i * 7 % 251) as u8).collect();
        Box::leak(Box::new([
            Entry {
                name: "crosstalk.exe",
                size: exe.len() as u64,
                data: pack(&exe),
            },
            Entry {
                name: "WebView2Loader.dll",
                size: 5,
                data: pack(b"dll!!"),
            },
        ]))
    }

    #[derive(Default)]
    struct Fake {
        free: Option<u64>,
        registered: Mutex<Option<(PathBuf, String)>>,
        shortcuts: Mutex<Option<(PathBuf, bool)>>,
        llm_checks: Mutex<Vec<PathBuf>>,
        llm_fails: bool,
    }
    impl Platform for Fake {
        fn free_space(&self, _: &Path) -> Option<u64> {
            self.free
        }
        fn same_volume(&self, _: &Path, _: &Path) -> bool {
            true
        }
        fn register(&self, dir: &Path, version: &str) -> Result<(), String> {
            *self.registered.lock().unwrap() = Some((dir.into(), version.into()));
            Ok(())
        }
        fn shortcuts(&self, exe: &Path, desktop: bool) -> Result<(), String> {
            *self.shortcuts.lock().unwrap() = Some((exe.into(), desktop));
            Ok(())
        }
        fn check_llm(&self, _exe: &Path, model: &Path) -> Result<String, String> {
            self.llm_checks.lock().unwrap().push(model.into());
            if self.llm_fails {
                Err("mémoire insuffisante".into())
            } else {
                Ok("vulkan".into())
            }
        }
    }

    /// TranslateGemma "installed": sparse file of the exact size + manifest.
    fn place_llm(models: &Path) {
        let m = llm::default_model();
        std::fs::create_dir_all(models.join(m.dir)).unwrap();
        std::fs::File::create(llm::model_path(models, m))
            .unwrap()
            .set_len(m.bytes)
            .unwrap();
        llm::write_manifest(
            &llm::manifest_path(models, m),
            &llm::Manifest {
                model: m.id.into(),
                file: m.file.into(),
                bytes: m.bytes,
                sha256: "ab".repeat(32),
                source: llm::url(m),
                verified_at: 0,
            },
        )
        .unwrap();
    }

    /// Serves every model as zeros of the right size, unless told to fail.
    struct Zeros {
        fail: bool,
        calls: AtomicUsize,
    }
    impl Fetcher for Zeros {
        fn fetch(
            &self,
            url: &str,
            offset: u64,
            sink: &mut dyn FnMut(&[u8]) -> Result<(), String>,
        ) -> Result<(), String> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            if self.fail {
                return Err("connexion impossible".into());
            }
            let size = downloads::missing_all(&PathBuf::from("/nonexistent"), WHISPER_MODELS, true)
                .into_iter()
                .find(|f| f.url == url)
                .unwrap()
                .bytes;
            let chunk = vec![0u8; 1 << 20];
            let mut left = size - offset;
            while left > 0 {
                let n = left.min(chunk.len() as u64) as usize;
                sink(&chunk[..n])?;
                left -= n as u64;
            }
            Ok(())
        }
    }

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("crosstalk-setup-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    fn installer(root: &Path, fetcher: Arc<dyn Fetcher>, platform: Arc<Fake>) -> Arc<Installer> {
        Installer::new(files(), root.join("models"), "0.2.0", fetcher, platform)
    }

    #[test]
    fn installs_files_and_registers_without_models() {
        let root = tmp("plain");
        let fake = Arc::new(Fake::default());
        let inst = installer(
            &root,
            Arc::new(Zeros {
                fail: false,
                calls: AtomicUsize::new(0),
            }),
            fake.clone(),
        );
        let dir = root.join("Crosstalk");
        let seen = Mutex::new(Vec::new());
        let r = inst.run(
            &Options {
                dir: dir.clone(),
                desktop: false,
                models: false,
            },
            &|p| seen.lock().unwrap().push(p),
        );
        assert_eq!(r, Ok(()));
        assert_eq!(
            std::fs::metadata(dir.join("crosstalk.exe")).unwrap().len(),
            700_000
        );
        assert_eq!(
            std::fs::read(dir.join("WebView2Loader.dll")).unwrap(),
            b"dll!!"
        );
        assert!(!dir.join(STAGING).exists());
        assert_eq!(
            *fake.registered.lock().unwrap(),
            Some((dir.clone(), "0.2.0".into()))
        );
        assert_eq!(
            *fake.shortcuts.lock().unwrap(),
            Some((dir.join("crosstalk.exe"), false))
        );
        assert!(inst.installed());
        let seen = seen.into_inner().unwrap();
        // Progress never goes back, phases follow the design's bounds, last one is "done".
        assert!(seen
            .windows(2)
            .all(|w| w[1].pct >= w[0].pct && w[1].phase >= w[0].phase));
        for p in &seen {
            let (lo, hi) =
                [(0.0, 12.0), (12.0, 62.0), (62.0, 88.0), (88.0, 100.0)][p.phase as usize];
            assert!(p.pct >= lo && p.pct <= hi, "{p:?}");
        }
        assert!(seen.last().unwrap().done);
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn downloads_the_missing_models_inside_the_extraction_phase() {
        let root = tmp("models");
        let fetcher = Arc::new(Zeros {
            fail: false,
            calls: AtomicUsize::new(0),
        });
        let inst = installer(&root, fetcher.clone(), Arc::new(Fake::default()));
        assert!(inst.model_bytes() > 700_000_000, "{}", inst.model_bytes());
        // Every model already present (sparse files of the exact size) but the VAD.
        place_llm(&inst.models_dir);
        for f in downloads::missing_all(&inst.models_dir, WHISPER_MODELS, true) {
            if f.path.ends_with(whisper::VAD_FILE) {
                continue;
            }
            std::fs::create_dir_all(f.path.parent().unwrap()).unwrap();
            std::fs::File::create(&f.path)
                .unwrap()
                .set_len(f.bytes)
                .unwrap();
        }
        assert_eq!(inst.model_bytes(), whisper::VAD_BYTES);
        let seen = Mutex::new(Vec::new());
        let r = inst.run(
            &Options {
                dir: root.join("app"),
                desktop: true,
                models: true,
            },
            &|p| seen.lock().unwrap().push(p),
        );
        assert_eq!(r, Ok(()));
        assert_eq!(inst.model_bytes(), 0);
        assert_eq!(fetcher.calls.load(Ordering::SeqCst), 1);
        let seen = seen.into_inner().unwrap();
        assert!(seen
            .iter()
            .any(|p| p.phase == 1 && p.detail.contains("Silero") && p.pct > 12.0 && p.pct <= 62.0));
        assert!(seen.windows(2).all(|w| w[1].pct >= w[0].pct));
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn failed_download_reports_an_error_and_leaves_nothing_installed() {
        let root = tmp("fail");
        let fake = Arc::new(Fake::default());
        let inst = installer(
            &root,
            Arc::new(Zeros {
                fail: true,
                calls: AtomicUsize::new(0),
            }),
            fake.clone(),
        );
        let dir = root.join("app");
        let last = Mutex::new(None);
        let r = inst.run(
            &Options {
                dir: dir.clone(),
                desktop: true,
                models: true,
            },
            &|p| *last.lock().unwrap() = Some(p),
        );
        assert!(
            matches!(r, Err(Stop::Failed(ref e)) if e.contains("connexion impossible")),
            "{r:?}"
        );
        let last = last.into_inner().unwrap().unwrap();
        assert!(last.error.is_some() && !last.done);
        assert!(!dir.join("crosstalk.exe").exists() && !dir.join(STAGING).exists());
        assert!(fake.registered.lock().unwrap().is_none());
        assert!(!inst.installed());
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn cancelling_keeps_the_previous_installation() {
        let root = tmp("cancel");
        let dir = root.join("app");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("crosstalk.exe"), b"old version").unwrap();
        let inst = installer(
            &root,
            Arc::new(Zeros {
                fail: false,
                calls: AtomicUsize::new(0),
            }),
            Arc::new(Fake::default()),
        );
        let me = inst.clone();
        let last = Mutex::new(None);
        let r = inst.run(
            &Options {
                dir: dir.clone(),
                desktop: true,
                models: false,
            },
            &|p| {
                if p.phase == 1 {
                    me.cancel();
                }
                *last.lock().unwrap() = Some(p);
            },
        );
        assert_eq!(r, Err(Stop::Cancelled));
        assert!(last.into_inner().unwrap().unwrap().cancelled);
        assert_eq!(
            std::fs::read(dir.join("crosstalk.exe")).unwrap(),
            b"old version"
        );
        assert!(!dir.join(STAGING).exists());
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn refuses_when_the_disk_is_too_small() {
        let root = tmp("space");
        let fake = Arc::new(Fake {
            free: Some(1_000),
            ..Default::default()
        });
        let inst = installer(
            &root,
            Arc::new(Zeros {
                fail: false,
                calls: AtomicUsize::new(0),
            }),
            fake,
        );
        let r = inst.run(
            &Options {
                dir: root.join("app"),
                desktop: true,
                models: false,
            },
            &|_| {},
        );
        assert!(
            matches!(r, Err(Stop::Failed(ref e)) if e.starts_with("Espace insuffisant")),
            "{r:?}"
        );
        assert!(!root.join("app").join("crosstalk.exe").exists());
        let _ = std::fs::remove_dir_all(&root);
    }
    fn files_with_engine() -> &'static [Entry] {
        Box::leak(Box::new([
            Entry {
                name: "crosstalk.exe",
                size: 3,
                data: pack(b"exe"),
            },
            Entry {
                name: "crosstalk-llm.exe",
                size: 3,
                data: pack(b"llm"),
            },
        ]))
    }

    #[test]
    fn the_translation_model_is_loaded_once_at_the_end() {
        for fails in [false, true] {
            let root = tmp(if fails { "llm-fail" } else { "llm-ok" });
            let fake = Arc::new(Fake {
                llm_fails: fails,
                ..Fake::default()
            });
            let inst = Installer::new(
                files_with_engine(),
                root.join("models"),
                "0.2.0",
                Arc::new(Zeros {
                    fail: false,
                    calls: AtomicUsize::new(0),
                }),
                fake.clone(),
            );
            place_llm(&inst.models_dir);
            let seen = Mutex::new(Vec::new());
            let r = inst.run(
                &Options {
                    dir: root.join("app"),
                    desktop: false,
                    models: false,
                },
                &|p| seen.lock().unwrap().push(p),
            );
            // A model that does not load never fails the installation.
            assert_eq!(r, Ok(()));
            assert_eq!(fake.llm_checks.lock().unwrap().len(), 1);
            let seen = seen.into_inner().unwrap();
            let last = seen.last().unwrap();
            assert!(last.done);
            if fails {
                assert!(last.warning.as_deref().unwrap().contains("Opus-MT"));
            } else {
                assert!(last.warning.is_none());
                assert!(seen.iter().any(|p| p.detail.contains("prêt (carte graphique")));
            }
            std::fs::remove_dir_all(&root).unwrap();
        }
    }

    #[test]
    fn without_a_checked_model_the_engine_is_not_started() {
        let root = tmp("llm-none");
        let fake = Arc::new(Fake::default());
        let inst = Installer::new(
            files_with_engine(),
            root.join("models"),
            "0.2.0",
            Arc::new(Zeros {
                fail: false,
                calls: AtomicUsize::new(0),
            }),
            fake.clone(),
        );
        let r = inst.run(
            &Options {
                dir: root.join("app"),
                desktop: false,
                models: false,
            },
            &|_| {},
        );
        assert_eq!(r, Ok(()));
        assert!(fake.llm_checks.lock().unwrap().is_empty());
        std::fs::remove_dir_all(&root).unwrap();
    }

    /// The real `crosstalk-llm --check` on the tiny model (CROSSTALK_LLM_EXE).
    #[test]
    fn real_engine_check_when_built() {
        let Some(exe) = std::env::var_os("CROSSTALK_LLM_EXE") else {
            eprintln!("CROSSTALK_LLM_EXE absent : test du vrai moteur sauté");
            return;
        };
        let data = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../src-llm/tests/data");
        let t = std::time::Duration::from_secs(60);
        assert_eq!(
            check_llm(Path::new(&exe), &data.join("tiny-gemma3.gguf"), t).unwrap(),
            "cpu"
        );
        let e = check_llm(Path::new(&exe), &data.join("absent.gguf"), t).unwrap_err();
        assert!(e.contains("introuvable"), "{e}");
    }

}
