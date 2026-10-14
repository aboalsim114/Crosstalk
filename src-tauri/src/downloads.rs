//! First-run download of the models: the installer stays small and the app
//! fetches what it needs (Silero VAD, the Whisper model, TranslateGemma,
//! Opus-MT ru/en → fr) from Hugging Face, once, with progress shown in the
//! UI. Every file is checked by its exact published size (and SHA-1 or
//! SHA-256 when published); an interrupted download resumes where it
//! stopped; a partial file (`.part`) is never taken for a model.
//!
//! TranslateGemma's GGUF (2.5 GB) is checked by SHA-256 and gets a manifest
//! once checked: a file of the right size without it is checked again
//! (never downloaded again if it is valid).

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use serde::Serialize;

use crate::speech::models as whisper;
use crate::translation::llm::catalog::{self as llm, LlmSpec, Manifest};
use crate::translation::models as opus;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sha256 {
    Pinned(&'static str),
    /// The SHA-256 the server announces for the file (Hugging Face: Git LFS
    /// object id in `X-Linked-Etag`).
    Announced,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileSpec {
    pub label: String,
    pub url: String,
    pub path: PathBuf,
    pub bytes: u64,
    pub sha1: Option<&'static str>,
    pub sha256: Option<Sha256>,
    /// LLM model: complete only with a matching manifest next to it.
    pub llm: Option<&'static LlmSpec>,
}

fn complete(spec: &FileSpec) -> bool {
    let size_ok = std::fs::metadata(&spec.path).is_ok_and(|m| m.len() == spec.bytes);
    match spec.llm {
        Some(m) => {
            size_ok
                && llm_dir(spec).is_some_and(|d| {
                    llm::read_manifest(&d, m).is_some_and(|man| llm::manifest_matches(&man, m))
                })
        }
        None => size_ok,
    }
}

/// Models directory of an LLM file (`<models>/<dir>/<file>`).
fn llm_dir(spec: &FileSpec) -> Option<PathBuf> {
    spec.path.parent()?.parent().map(Path::to_path_buf)
}

/// TranslateGemma's GGUF as a download.
pub fn llm_file(dir: &Path, m: &'static LlmSpec) -> FileSpec {
    FileSpec {
        label: format!("Traduction ({} {})", m.name, m.quant),
        url: llm::url(m),
        path: llm::model_path(dir, m),
        bytes: m.bytes,
        sha1: None,
        sha256: Some(m.sha256.map_or(Sha256::Announced, Sha256::Pinned)),
        llm: Some(m),
    }
}

/// Files the app needs and does not have yet: VAD, the chosen Whisper model
/// and, when translation is on, both Opus-MT models.
pub fn missing(dir: &Path, whisper_model: &str, translation: bool) -> Vec<FileSpec> {
    missing_all(dir, &[whisper_model], translation)
}

/// Same, for several Whisper models (the installer fetches every model the
/// app can run on the CPU, so that changing model never downloads anything).
pub fn missing_all(dir: &Path, whisper_models: &[&str], translation: bool) -> Vec<FileSpec> {
    let mut all = vec![FileSpec {
        label: "Détection de parole (Silero)".into(),
        url: whisper::VAD_URL.into(),
        path: dir.join(whisper::VAD_FILE),
        bytes: whisper::VAD_BYTES,
        sha1: None,
        sha256: None,
        llm: None,
    }];
    for m in whisper_models.iter().filter_map(|id| whisper::spec(id)) {
        all.push(FileSpec {
            label: format!("Transcription (Whisper {})", m.id),
            url: whisper::url(m),
            path: dir.join(m.file),
            bytes: m.bytes,
            sha1: Some(m.sha1),
            sha256: None,
            llm: None,
        });
    }
    if translation {
        all.push(llm_file(dir, llm::default_model()));
        for p in opus::CATALOG {
            for &(file, bytes) in &p.files {
                all.push(FileSpec {
                    label: format!("Traduction {} → français", p.source.to_uppercase()),
                    url: format!("https://huggingface.co/{}/resolve/main/{file}", p.repo),
                    path: opus::pair_dir(dir, p).join(file),
                    bytes,
                    sha1: None,
                    sha256: None,
                    llm: None,
                });
            }
        }
        for p in opus::REPLY {
            for &(file, bytes) in &p.files {
                all.push(FileSpec {
                    label: format!("Réponse français → {}", p.target.to_uppercase()),
                    url: format!("https://huggingface.co/{}/resolve/main/{file}", p.repo),
                    path: opus::pair_dir(dir, p).join(file),
                    bytes,
                    sha1: None,
                    sha256: None,
                    llm: None,
                });
            }
        }
    }
    all.retain(|f| !complete(f));
    all
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum DownloadState {
    Idle,
    Downloading,
    Done,
    Error,
    Cancelled,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadStatus {
    pub state: DownloadState,
    /// What is being downloaded now.
    pub current: Option<String>,
    pub done_bytes: u64,
    pub total_bytes: u64,
    /// Files still missing (0 when everything is installed).
    pub missing_files: usize,
    pub missing_bytes: u64,
    pub error: Option<String>,
    /// Average speed over the last seconds (bytes/s).
    #[serde(default)]
    pub bytes_per_sec: u64,
    /// Checking a downloaded file's fingerprint (no bytes move meanwhile).
    #[serde(default)]
    pub verifying: bool,
}

pub trait DownloadEvents: Send + Sync {
    fn progress(&self, status: &DownloadStatus);
    /// Called once all files are in place (rescan the engines).
    fn finished(&self);
}

/// Fetches `url` (from `offset`) and writes the bytes; tests use a local server.
pub trait Fetcher: Send + Sync {
    fn fetch(
        &self,
        url: &str,
        offset: u64,
        sink: &mut dyn FnMut(&[u8]) -> Result<(), String>,
    ) -> Result<(), String>;

    /// SHA-256 the server announces for `url`, without downloading it.
    fn announced_sha256(&self, _url: &str) -> Result<Option<String>, String> {
        Ok(None)
    }
}

/// `"<hex>"` or `W/"<hex>"` (ETag forms) to the SHA-256, if it is one.
pub fn sha256_from_etag(v: &str) -> Option<String> {
    let v = v.trim().trim_start_matches("W/").trim_matches('"');
    llm::is_sha256(v).then(|| v.to_ascii_lowercase())
}

pub struct HttpFetcher;

impl Fetcher for HttpFetcher {
    fn announced_sha256(&self, url: &str) -> Result<Option<String>, String> {
        // The first answer (before the redirect to the CDN) carries the LFS id.
        let agent = ureq::AgentBuilder::new()
            .timeout_connect(std::time::Duration::from_secs(15))
            .timeout_read(std::time::Duration::from_secs(30))
            .redirects(0)
            .build();
        let resp = match agent.head(url).set("User-Agent", "Crosstalk").call() {
            Ok(r) => r,
            Err(ureq::Error::Status(code, r)) if (300..400).contains(&code) => r,
            Err(ureq::Error::Status(code, _)) => {
                return Err(format!("le serveur a répondu {code}"))
            }
            Err(ureq::Error::Transport(t)) => {
                return Err(format!("connexion impossible ({})", t.kind()))
            }
        };
        Ok(["x-linked-etag", "etag"]
            .iter()
            .find_map(|h| resp.header(h).and_then(sha256_from_etag)))
    }

    fn fetch(
        &self,
        url: &str,
        offset: u64,
        sink: &mut dyn FnMut(&[u8]) -> Result<(), String>,
    ) -> Result<(), String> {
        let agent = ureq::AgentBuilder::new()
            .timeout_connect(std::time::Duration::from_secs(15))
            .timeout_read(std::time::Duration::from_secs(60))
            .redirects(8)
            .build();
        let mut req = agent.get(url).set("User-Agent", "Crosstalk");
        if offset > 0 {
            req = req.set("Range", &format!("bytes={offset}-"));
        }
        let resp = req.call().map_err(|e| match e {
            ureq::Error::Status(code, _) => format!("le serveur a répondu {code}"),
            ureq::Error::Transport(t) => format!("connexion impossible ({})", t.kind()),
        })?;
        if offset > 0 && resp.status() != 206 {
            return Err("reprise refusée par le serveur".into());
        }
        let mut body = resp.into_reader();
        let mut buf = vec![0u8; 256 * 1024];
        loop {
            let n = body
                .read(&mut buf)
                .map_err(|e| format!("lecture interrompue : {e}"))?;
            if n == 0 {
                return Ok(());
            }
            sink(&buf[..n])?;
        }
    }
}

/// Free bytes on the volume holding `dir` (its nearest existing parent).
pub type FreeSpace = Box<dyn Fn(&Path) -> Option<u64> + Send + Sync>;

pub struct Downloader {
    fetcher: Arc<dyn Fetcher>,
    free_space: FreeSpace,
    events: Arc<dyn DownloadEvents>,
    status: Mutex<DownloadStatus>,
    running: AtomicBool,
    cancel: AtomicBool,
}

impl Downloader {
    pub fn new(fetcher: Arc<dyn Fetcher>, events: Arc<dyn DownloadEvents>) -> Arc<Self> {
        Arc::new(Self {
            fetcher,
            events,
            status: Mutex::new(DownloadStatus {
                state: DownloadState::Idle,
                current: None,
                done_bytes: 0,
                total_bytes: 0,
                missing_files: 0,
                missing_bytes: 0,
                error: None,
                bytes_per_sec: 0,
                verifying: false,
            }),
            running: AtomicBool::new(false),
            cancel: AtomicBool::new(false),
            free_space: Box::new(free_space),
        })
    }

    /// Same, with another view of the disk (tests: a full disk).
    pub fn with_free_space(
        fetcher: Arc<dyn Fetcher>,
        events: Arc<dyn DownloadEvents>,
        free_space: FreeSpace,
    ) -> Arc<Self> {
        let mut d = Self::new(fetcher, events);
        Arc::get_mut(&mut d).expect("fresh").free_space = free_space;
        d
    }

    /// Current state, with what is still missing for this configuration.
    pub fn status(&self, dir: &Path, whisper_model: &str, translation: bool) -> DownloadStatus {
        let todo = missing(dir, whisper_model, translation);
        let mut st = self.status.lock().unwrap().clone();
        if !self.running.load(Ordering::SeqCst) {
            st.missing_files = todo.len();
            st.missing_bytes = todo.iter().map(|f| f.bytes).sum();
        }
        st
    }

    /// Starts downloading what is missing, on a background thread. Returns
    /// false when a download already runs or nothing is missing.
    pub fn start(self: &Arc<Self>, dir: PathBuf, whisper_model: &str, translation: bool) -> bool {
        self.start_all(dir, &[whisper_model], translation)
    }

    /// [`Downloader::start`] for several Whisper models.
    pub fn start_all(
        self: &Arc<Self>,
        dir: PathBuf,
        whisper_models: &[&str],
        translation: bool,
    ) -> bool {
        let files = missing_all(&dir, whisper_models, translation);
        if files.is_empty() || self.running.swap(true, Ordering::SeqCst) {
            return false;
        }
        self.cancel.store(false, Ordering::SeqCst);
        let me = self.clone();
        std::thread::Builder::new()
            .name("models-download".into())
            .spawn(move || me.run(files))
            .expect("download thread");
        true
    }

    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::SeqCst);
    }

    fn publish(&self, f: impl FnOnce(&mut DownloadStatus)) {
        let st = {
            let mut s = self.status.lock().unwrap();
            f(&mut s);
            s.clone()
        };
        self.events.progress(&st);
    }

    fn run(&self, files: Vec<FileSpec>) {
        let total: u64 = files.iter().map(|f| f.bytes).sum();
        let already: u64 = files.iter().map(partial_len).sum();
        self.publish(|s| {
            *s = DownloadStatus {
                state: DownloadState::Downloading,
                current: None,
                done_bytes: already,
                total_bytes: total,
                missing_files: files.len(),
                missing_bytes: total,
                error: None,
                bytes_per_sec: 0,
                verifying: false,
            }
        });
        let mut result = self.check_space(&files);
        for (i, f) in files.iter().enumerate() {
            if result.is_err() {
                break;
            }
            self.publish(|s| {
                s.current = Some(f.label.clone());
                s.missing_files = files.len() - i;
            });
            if let Err(e) = self.download(f) {
                result = Err(e);
                break;
            }
        }
        let cancelled = self.cancel.load(Ordering::SeqCst);
        self.publish(|s| {
            s.current = None;
            match &result {
                Ok(()) => {
                    s.state = DownloadState::Done;
                    s.verifying = false;
                    s.missing_files = 0;
                    s.missing_bytes = 0;
                }
                Err(_) if cancelled => s.state = DownloadState::Cancelled,
                Err(e) => {
                    s.state = DownloadState::Error;
                    s.error = Some(e.clone());
                }
            }
        });
        self.running.store(false, Ordering::SeqCst);
        if result.is_ok() {
            self.events.finished();
        }
    }

    /// Refuses to start when the files cannot fit (with 256 MB to spare).
    fn check_space(&self, files: &[FileSpec]) -> Result<(), String> {
        let Some(dir) = files.first().and_then(|f| f.path.parent()) else {
            return Ok(());
        };
        let need: u64 = files
            .iter()
            .map(|f| f.bytes.saturating_sub(partial_len(f)))
            .sum();
        match (self.free_space)(dir) {
            Some(free) if free < need + (256 << 20) => Err(format!(
                "Espace disque insuffisant : {:.1} Go libres, {:.1} Go nécessaires pour les modèles.",
                free as f64 / 1e9,
                (need + (256 << 20)) as f64 / 1e9
            )),
            _ => Ok(()),
        }
    }

    /// Expected SHA-256 of `f`, if it has one.
    fn expected_sha256(&self, f: &FileSpec) -> Result<Option<String>, String> {
        match f.sha256 {
            None => Ok(None),
            Some(Sha256::Pinned(h)) => Ok(Some(h.to_ascii_lowercase())),
            Some(Sha256::Announced) => self
                .fetcher
                .announced_sha256(&f.url)?
                .map(|h| Some(h.to_ascii_lowercase()))
                .ok_or_else(|| {
                    format!("{} : empreinte SHA-256 non publiée par le serveur", f.label)
                }),
        }
    }

    /// Checks `path` against the expected SHA-256 and records the manifest.
    /// Returns false (file left in place) when the fingerprint differs.
    fn verify_llm(&self, f: &FileSpec, path: &Path, expected: &str) -> Result<bool, String> {
        self.publish(|s| s.verifying = true);
        let got = sha256_of(path, &self.cancel)?;
        self.publish(|s| s.verifying = false);
        if got != expected {
            return Ok(false);
        }
        if let (Some(m), Some(dir)) = (f.llm, llm_dir(f)) {
            llm::write_manifest(
                &llm::manifest_path(&dir, m),
                &Manifest {
                    model: m.id.into(),
                    file: m.file.into(),
                    bytes: m.bytes,
                    sha256: got,
                    source: f.url.clone(),
                    verified_at: std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map_or(0, |d| d.as_secs()),
                },
            )?;
        }
        Ok(true)
    }

    fn download(&self, f: &FileSpec) -> Result<(), String> {
        if let Some(parent) = f.path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("dossier des modèles : {e}"))?;
        }
        // A model already there (copied by hand, or manifest lost): checked,
        // never downloaded again when valid.
        if f.llm.is_some() && std::fs::metadata(&f.path).is_ok_and(|m| m.len() == f.bytes) {
            if let Some(expected) = self.expected_sha256(f)? {
                if self.verify_llm(f, &f.path, &expected)? {
                    self.publish(|s| s.done_bytes += f.bytes);
                    return Ok(());
                }
                let _ = std::fs::remove_file(&f.path);
            }
        }
        let part = part_path(&f.path);
        let mut offset = partial_len(f);
        if offset > f.bytes {
            let _ = std::fs::remove_file(&part);
            offset = 0;
        }
        if offset < f.bytes {
            let mut out = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&part)
                .map_err(|e| format!("écriture impossible : {e}"))?;
            let mut written = offset;
            let mut last = std::time::Instant::now();
            let mut window = (std::time::Instant::now(), 0u64);
            let resumed = self.fetcher.fetch(&f.url, offset, &mut |chunk| {
                if self.cancel.load(Ordering::SeqCst) {
                    return Err("téléchargement annulé".into());
                }
                written += chunk.len() as u64;
                if written > f.bytes {
                    return Err("fichier plus grand que prévu".into());
                }
                out.write_all(chunk)
                    .map_err(|e| format!("écriture impossible : {e}"))?;
                window.1 += chunk.len() as u64;
                if last.elapsed().as_millis() >= 200 {
                    last = std::time::Instant::now();
                    let n = chunk.len() as u64;
                    let secs = window.0.elapsed().as_secs_f64();
                    let speed = (secs >= 1.0).then(|| (window.1 as f64 / secs) as u64);
                    if secs >= 3.0 {
                        window = (std::time::Instant::now(), 0);
                    }
                    self.publish(|s| {
                        s.done_bytes += n;
                        if let Some(v) = speed {
                            s.bytes_per_sec = v;
                        }
                    });
                    return Ok(());
                }
                let n = chunk.len() as u64;
                self.status.lock().unwrap().done_bytes += n;
                Ok(())
            });
            out.flush()
                .map_err(|e| format!("écriture impossible : {e}"))?;
            resumed.map_err(|e| format!("{} : {e}", f.label))?;
        }
        let len = std::fs::metadata(&part).map(|m| m.len()).unwrap_or(0);
        if len != f.bytes {
            return Err(format!(
                "{} : taille inattendue ({len} au lieu de {} octets)",
                f.label, f.bytes
            ));
        }
        if let Some(expected) = f.sha1 {
            if sha1_of(&part)? != expected {
                let _ = std::fs::remove_file(&part);
                return Err(format!(
                    "{} : empreinte SHA-1 invalide, fichier supprimé",
                    f.label
                ));
            }
        }
        if let Some(expected) = self.expected_sha256(f)? {
            if !self.verify_llm(f, &part, &expected)? {
                let _ = std::fs::remove_file(&part);
                return Err(format!(
                    "{} : empreinte SHA-256 invalide (téléchargement corrompu), fichier supprimé",
                    f.label
                ));
            }
        }
        std::fs::rename(&part, &f.path).map_err(|e| format!("installation impossible : {e}"))?;
        self.publish(|_| {});
        Ok(())
    }
}

fn part_path(path: &Path) -> PathBuf {
    let mut p = path.as_os_str().to_owned();
    p.push(".part");
    PathBuf::from(p)
}

pub fn partial_len(f: &FileSpec) -> u64 {
    std::fs::metadata(part_path(&f.path))
        .map(|m| m.len())
        .unwrap_or(0)
}

pub fn sha256_of(path: &Path, cancel: &AtomicBool) -> Result<String, String> {
    use sha2::Digest;
    let mut file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut h = sha2::Sha256::new();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        if cancel.load(Ordering::SeqCst) {
            return Err("vérification annulée".into());
        }
        let n = file.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            let d = h.finalize();
            return Ok(d.iter().map(|b| format!("{b:02x}")).collect());
        }
        h.update(&buf[..n]);
    }
}

/// "Réparer": checks an installed LLM model again, byte by byte. A file that
/// does not match is deleted with its manifest (the next download replaces
/// it); `Ok` returns the recorded manifest.
pub fn verify_installed(
    dir: &Path,
    m: &'static LlmSpec,
    fetcher: &dyn Fetcher,
) -> Result<Manifest, String> {
    let f = llm_file(dir, m);
    if !std::fs::metadata(&f.path).is_ok_and(|x| x.len() == m.bytes) {
        let _ = std::fs::remove_file(llm::manifest_path(dir, m));
        return Err("modèle absent ou incomplet : téléchargez-le".into());
    }
    // Known fingerprint: pinned, else the recorded one, else the server's.
    let expected = match m.sha256 {
        Some(h) => h.to_ascii_lowercase(),
        None => match llm::read_manifest(dir, m).filter(|x| llm::manifest_matches(x, m)) {
            Some(man) => man.sha256,
            None => fetcher
                .announced_sha256(&f.url)?
                .ok_or("empreinte SHA-256 non publiée par le serveur")?
                .to_ascii_lowercase(),
        },
    };
    let got = sha256_of(&f.path, &AtomicBool::new(false))?;
    if got != expected {
        let _ = std::fs::remove_file(&f.path);
        let _ = std::fs::remove_file(llm::manifest_path(dir, m));
        return Err("modèle corrompu : supprimé, téléchargez-le à nouveau".into());
    }
    let man = Manifest {
        model: m.id.into(),
        file: m.file.into(),
        bytes: m.bytes,
        sha256: got,
        source: f.url.clone(),
        verified_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs()),
    };
    llm::write_manifest(&llm::manifest_path(dir, m), &man)?;
    Ok(man)
}

/// Free bytes for `dir` (or its nearest existing parent).
#[cfg(windows)]
pub fn free_space(dir: &Path) -> Option<u64> {
    use std::os::windows::ffi::OsStrExt;
    extern "system" {
        fn GetDiskFreeSpaceExW(
            dir: *const u16,
            free_to_caller: *mut u64,
            total: *mut u64,
            free: *mut u64,
        ) -> i32;
    }
    let existing = dir.ancestors().find(|p| p.exists())?;
    let wide: Vec<u16> = existing.as_os_str().encode_wide().chain(Some(0)).collect();
    let (mut avail, mut total, mut free) = (0u64, 0u64, 0u64);
    let ok = unsafe { GetDiskFreeSpaceExW(wide.as_ptr(), &mut avail, &mut total, &mut free) };
    (ok != 0).then_some(avail)
}

#[cfg(not(windows))]
pub fn free_space(_dir: &Path) -> Option<u64> {
    None
}

fn sha1_of(path: &Path) -> Result<String, String> {
    let mut file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut h = sha1_smol::Sha1::new();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = file.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            return Ok(h.digest().to_string());
        }
        h.update(&buf[..n]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::mpsc::{channel, Sender};

    /// Serves fixed bodies by URL, honouring the offset like a Range request.
    struct Fake {
        bodies: HashMap<String, Vec<u8>>,
        fail_after: Mutex<Option<usize>>,
    }
    /// A Fake that also announces SHA-256 fingerprints, and counts fetches.
    struct Announcing {
        inner: Fake,
        sha256: HashMap<String, String>,
        fetches: std::sync::atomic::AtomicUsize,
    }
    impl Fetcher for Announcing {
        fn fetch(
            &self,
            url: &str,
            offset: u64,
            sink: &mut dyn FnMut(&[u8]) -> Result<(), String>,
        ) -> Result<(), String> {
            self.fetches.fetch_add(1, Ordering::SeqCst);
            self.inner.fetch(url, offset, sink)
        }
        fn announced_sha256(&self, url: &str) -> Result<Option<String>, String> {
            Ok(self.sha256.get(url).cloned())
        }
    }
    impl Fetcher for Fake {
        fn fetch(
            &self,
            url: &str,
            offset: u64,
            sink: &mut dyn FnMut(&[u8]) -> Result<(), String>,
        ) -> Result<(), String> {
            let body = self.bodies.get(url).ok_or("404")?;
            let mut sent = 0;
            for chunk in body[offset as usize..].chunks(7) {
                if let Some(limit) = *self.fail_after.lock().unwrap() {
                    if sent >= limit {
                        return Err("coupure réseau".into());
                    }
                }
                sink(chunk)?;
                sent += chunk.len();
            }
            Ok(())
        }
    }

    struct Events(Mutex<Sender<DownloadStatus>>);
    impl DownloadEvents for Events {
        fn progress(&self, s: &DownloadStatus) {
            let _ = self.0.lock().unwrap().send(s.clone());
        }
        fn finished(&self) {}
    }

    fn spec(dir: &Path, name: &str, body: &[u8], sha1: Option<&'static str>) -> FileSpec {
        FileSpec {
            label: name.into(),
            url: format!("https://x/{name}"),
            path: dir.join(name),
            bytes: body.len() as u64,
            sha1,
            sha256: None,
            llm: None,
        }
    }

    fn dir(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("crosstalk-dl-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn wait_end(rx: &std::sync::mpsc::Receiver<DownloadStatus>) -> DownloadStatus {
        loop {
            let s = rx
                .recv_timeout(std::time::Duration::from_secs(5))
                .expect("progress");
            if s.state != DownloadState::Downloading {
                return s;
            }
        }
    }

    #[test]
    fn downloads_checks_and_installs_files() {
        let d = dir("ok");
        let body = b"hello world, this is a model".to_vec();
        let sha1 = {
            let mut h = sha1_smol::Sha1::new();
            h.update(&body);
            h.digest().to_string()
        };
        let f = spec(&d, "a.bin", &body, Some(Box::leak(sha1.into_boxed_str())));
        let (tx, rx) = channel();
        let dl = Downloader::new(
            Arc::new(Fake {
                bodies: [(f.url.clone(), body.clone())].into(),
                fail_after: Mutex::new(None),
            }),
            Arc::new(Events(Mutex::new(tx))),
        );
        dl.run(vec![f.clone()]);
        let end = wait_end(&rx);
        assert_eq!(end.state, DownloadState::Done);
        assert_eq!(std::fs::read(&f.path).unwrap(), body);
        assert!(!part_path(&f.path).exists());
    }

    #[test]
    fn a_wrong_sha1_deletes_the_file() {
        let d = dir("sha");
        let body = b"tampered".to_vec();
        let f = spec(
            &d,
            "b.bin",
            &body,
            Some("0000000000000000000000000000000000000000"),
        );
        let (tx, rx) = channel();
        let dl = Downloader::new(
            Arc::new(Fake {
                bodies: [(f.url.clone(), body)].into(),
                fail_after: Mutex::new(None),
            }),
            Arc::new(Events(Mutex::new(tx))),
        );
        dl.run(vec![f.clone()]);
        let end = wait_end(&rx);
        assert_eq!(end.state, DownloadState::Error);
        assert!(end.error.unwrap().contains("SHA-1"));
        assert!(!f.path.exists() && !part_path(&f.path).exists());
    }

    #[test]
    fn an_interrupted_download_resumes_where_it_stopped() {
        let d = dir("resume");
        let body: Vec<u8> = (0..200u8).collect();
        let f = spec(&d, "c.bin", &body, None);
        let fake = Arc::new(Fake {
            bodies: [(f.url.clone(), body.clone())].into(),
            fail_after: Mutex::new(Some(70)),
        });
        let (tx, rx) = channel();
        let dl = Downloader::new(fake.clone(), Arc::new(Events(Mutex::new(tx))));
        dl.run(vec![f.clone()]);
        assert_eq!(wait_end(&rx).state, DownloadState::Error);
        let partial = std::fs::metadata(part_path(&f.path)).unwrap().len();
        assert!(partial > 0 && partial < 200);
        *fake.fail_after.lock().unwrap() = None;
        dl.run(vec![f.clone()]);
        assert_eq!(wait_end(&rx).state, DownloadState::Done);
        assert_eq!(std::fs::read(&f.path).unwrap(), body);
    }

    #[test]
    fn cancel_stops_without_installing() {
        let d = dir("cancel");
        let body = vec![1u8; 100];
        let f = spec(&d, "d.bin", &body, None);
        let (tx, rx) = channel();
        let dl = Downloader::new(
            Arc::new(Fake {
                bodies: [(f.url.clone(), body)].into(),
                fail_after: Mutex::new(None),
            }),
            Arc::new(Events(Mutex::new(tx))),
        );
        dl.cancel();
        dl.run(vec![f.clone()]);
        assert_eq!(wait_end(&rx).state, DownloadState::Cancelled);
        assert!(!f.path.exists());
    }

    /// Real HTTP: redirect (like Hugging Face → CDN) then a Range request.
    #[test]
    fn http_fetcher_follows_redirects_and_resumes_with_range() {
        use std::io::{BufRead, BufReader};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let body: Vec<u8> = (0..=255u8).cycle().take(5000).collect();
        let served = body.clone();
        let server = std::thread::spawn(move || {
            let mut ranges = Vec::new();
            for _ in 0..2 {
                let (mut sock, _) = listener.accept().unwrap();
                let mut reader = BufReader::new(sock.try_clone().unwrap());
                let mut first = String::new();
                reader.read_line(&mut first).unwrap();
                let mut range = None;
                loop {
                    let mut line = String::new();
                    reader.read_line(&mut line).unwrap();
                    if line == "\r\n" || line.is_empty() {
                        break;
                    }
                    if let Some(v) = line.to_ascii_lowercase().strip_prefix("range: bytes=") {
                        range = v.trim().trim_end_matches('-').parse::<usize>().ok();
                    }
                }
                if first.starts_with("GET /model ") {
                    write!(sock, "HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1:{port}/cdn/model\r\nContent-Length: 0\r\n\r\n").unwrap();
                } else {
                    let from = range.unwrap_or(0);
                    let part = &served[from..];
                    write!(sock, "HTTP/1.1 206 Partial Content\r\nContent-Length: {}\r\nContent-Range: bytes {from}-{}/{}\r\n\r\n", part.len(), served.len() - 1, served.len()).unwrap();
                    sock.write_all(part).unwrap();
                    ranges.push(from);
                }
            }
            ranges
        });
        let mut got = Vec::new();
        HttpFetcher
            .fetch(&format!("http://127.0.0.1:{port}/model"), 1234, &mut |c| {
                got.extend_from_slice(c);
                Ok(())
            })
            .unwrap();
        assert_eq!(server.join().unwrap(), vec![1234]);
        assert_eq!(got, body[1234..]);
    }

    #[test]
    fn the_starter_pack_is_vad_whisper_and_the_translation_models() {
        let d = dir("pack");
        let all = missing(&d, "base", true);
        // VAD + Whisper, TranslateGemma, ru/en → fr, then fr → en/ru (replies).
        assert_eq!(all.len(), 2 + 1 + 10 + 10);
        assert_eq!(all.iter().filter(|f| f.llm.is_some()).count(), 1);
        assert_eq!(
            all.iter()
                .filter(|f| f.label.starts_with("Réponse"))
                .count(),
            10
        );
        let mb: u64 = all.iter().map(|f| f.bytes).sum::<u64>() / 1_000_000;
        assert!((3860..3910).contains(&mb), "{mb} Mo");
        assert!(all
            .iter()
            .all(|f| f.url.starts_with("https://huggingface.co/")));
        assert_eq!(missing(&d, "base", false).len(), 2);
        std::fs::write(
            d.join(whisper::VAD_FILE),
            vec![0u8; whisper::VAD_BYTES as usize],
        )
        .unwrap();
        assert_eq!(missing(&d, "base", false).len(), 1);
    }

    #[test]
    fn the_installer_pack_holds_every_cpu_model() {
        let d = dir("all");
        let all = missing_all(&d, &["base", "small"], true);
        assert_eq!(all.len(), 3 + 1 + 20);
        let mb: u64 = all.iter().map(|f| f.bytes).sum::<u64>() / 1_000_000;
        assert!((4290..4440).contains(&mb), "{mb} Mo");
        assert_eq!(all.iter().filter(|f| f.sha256.is_some()).count(), 1);
        assert_eq!(all.iter().filter(|f| f.sha1.is_some()).count(), 2);
    }
    fn sha256_hex(b: &[u8]) -> String {
        use sha2::Digest;
        sha2::Sha256::digest(b)
            .iter()
            .map(|x| format!("{x:02x}"))
            .collect()
    }

    fn tiny_llm(bytes: usize) -> &'static LlmSpec {
        Box::leak(Box::new(LlmSpec {
            bytes: bytes as u64,
            dir: "tg",
            file: "m.gguf",
            ..llm::TRANSLATEGEMMA_4B
        }))
    }

    fn llm_setup(
        tag: &str,
        body: &[u8],
        announced: Option<String>,
    ) -> (PathBuf, FileSpec, Arc<Announcing>) {
        let d = dir(tag);
        let m = tiny_llm(body.len());
        let f = llm_file(&d, m);
        let fake = Arc::new(Announcing {
            inner: Fake {
                bodies: [(f.url.clone(), body.to_vec())].into(),
                fail_after: Mutex::new(None),
            },
            sha256: announced.map(|h| (f.url.clone(), h)).into_iter().collect(),
            fetches: Default::default(),
        });
        (d, f, fake)
    }

    #[test]
    fn the_llm_model_is_checked_by_sha256_and_gets_a_manifest() {
        let body: Vec<u8> = (0..5000u32).map(|i| (i % 251) as u8).collect();
        let (d, f, fake) = llm_setup("llm-ok", &body, Some(sha256_hex(&body).to_uppercase()));
        assert!(!complete(&f));
        let (tx, rx) = channel();
        let dl = Downloader::new(fake, Arc::new(Events(Mutex::new(tx))));
        dl.run(vec![f.clone()]);
        assert_eq!(wait_end(&rx).state, DownloadState::Done);
        let man = llm::read_manifest(&d, f.llm.unwrap()).unwrap();
        assert_eq!(man.sha256, sha256_hex(&body));
        assert_eq!(man.bytes, 5000);
        assert!(complete(&f));
        assert_eq!(
            llm::install_state(&d, f.llm.unwrap()),
            llm::Install::Verified {
                sha256: sha256_hex(&body)
            }
        );
    }

    #[test]
    fn a_corrupted_llm_download_is_deleted() {
        let body = vec![7u8; 3000];
        let (d, f, fake) = llm_setup("llm-bad", &body, Some("ab".repeat(32)));
        let (tx, rx) = channel();
        let dl = Downloader::new(fake, Arc::new(Events(Mutex::new(tx))));
        dl.run(vec![f.clone()]);
        let end = wait_end(&rx);
        assert_eq!(end.state, DownloadState::Error);
        assert!(end.error.unwrap().contains("SHA-256"));
        assert!(!f.path.exists() && !part_path(&f.path).exists());
        assert!(llm::read_manifest(&d, f.llm.unwrap()).is_none());
    }

    #[test]
    fn without_a_published_fingerprint_the_model_is_not_installed() {
        let body = vec![1u8; 1000];
        let (_d, f, fake) = llm_setup("llm-nohash", &body, None);
        let (tx, rx) = channel();
        let dl = Downloader::new(fake, Arc::new(Events(Mutex::new(tx))));
        dl.run(vec![f.clone()]);
        let end = wait_end(&rx);
        assert_eq!(end.state, DownloadState::Error);
        assert!(end.error.unwrap().contains("non publiée"));
        assert!(!f.path.exists());
    }

    #[test]
    fn a_valid_model_already_on_disk_is_never_downloaded_again() {
        let body: Vec<u8> = (0..4096u32).map(|i| (i * 3 % 256) as u8).collect();
        let (d, f, fake) = llm_setup("llm-present", &body, Some(sha256_hex(&body)));
        std::fs::create_dir_all(f.path.parent().unwrap()).unwrap();
        std::fs::write(&f.path, &body).unwrap();
        // Right size, no manifest (copied by hand, or a reinstall).
        assert!(!complete(&f));
        let (tx, rx) = channel();
        let dl = Downloader::new(fake.clone(), Arc::new(Events(Mutex::new(tx))));
        dl.run(vec![f.clone()]);
        assert_eq!(wait_end(&rx).state, DownloadState::Done);
        assert_eq!(fake.fetches.load(Ordering::SeqCst), 0);
        assert!(complete(&f));
        assert!(missing_all(&d, &[], false).iter().all(|x| x.llm.is_none()));
    }

    #[test]
    fn a_full_disk_stops_before_downloading() {
        let body = vec![0u8; 2000];
        let (_d, f, fake) = llm_setup("llm-full", &body, Some(sha256_hex(&body)));
        let (tx, rx) = channel();
        let dl = Downloader::with_free_space(
            fake.clone(),
            Arc::new(Events(Mutex::new(tx))),
            Box::new(|_| Some(10_000)),
        );
        dl.run(vec![f.clone()]);
        let end = wait_end(&rx);
        assert_eq!(end.state, DownloadState::Error);
        assert!(end.error.unwrap().contains("Espace disque insuffisant"));
        assert_eq!(fake.fetches.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn repair_detects_a_corrupted_model() {
        let body: Vec<u8> = (0..3000u32).map(|i| (i % 7) as u8).collect();
        let (d, f, fake) = llm_setup("llm-repair", &body, Some(sha256_hex(&body)));
        let m = f.llm.unwrap();
        let (tx, rx) = channel();
        let dl = Downloader::new(fake.clone(), Arc::new(Events(Mutex::new(tx))));
        dl.run(vec![f.clone()]);
        assert_eq!(wait_end(&rx).state, DownloadState::Done);
        assert!(verify_installed(&d, m, &*fake).is_ok());
        // One flipped byte, same size: still "Verified" by size + manifest,
        // only a full check finds it.
        let mut bad = body.clone();
        bad[100] ^= 0xff;
        std::fs::write(&f.path, &bad).unwrap();
        assert!(complete(&f));
        let e = verify_installed(&d, m, &*fake).unwrap_err();
        assert!(e.contains("corrompu"), "{e}");
        assert!(!f.path.exists());
        assert_eq!(llm::install_state(&d, m), llm::Install::Missing);
    }

    #[test]
    fn etag_forms() {
        let h = "AB".repeat(32);
        assert_eq!(
            sha256_from_etag(&format!("\"{h}\"")),
            Some(h.to_lowercase())
        );
        assert_eq!(
            sha256_from_etag(&format!("W/\"{h}\"")),
            Some(h.to_lowercase())
        );
        assert_eq!(sha256_from_etag("\"abc\""), None);
    }

    /// Real HTTP: the fingerprint is read from the redirect, as Hugging Face sends it.
    #[test]
    fn http_fetcher_reads_the_announced_sha256() {
        use std::io::{BufRead, BufReader};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let h = "cd".repeat(32);
        let sent = h.clone();
        let server = std::thread::spawn(move || {
            let (mut sock, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(sock.try_clone().unwrap());
            let mut first = String::new();
            reader.read_line(&mut first).unwrap();
            loop {
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                if line == "\r\n" || line.is_empty() {
                    break;
                }
            }
            assert!(first.starts_with("HEAD /m.gguf "), "{first}");
            write!(
                sock,
                "HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1:1/cdn\r\nX-Linked-Etag: \"{sent}\"\r\nContent-Length: 0\r\n\r\n"
            )
            .unwrap();
        });
        let got = HttpFetcher
            .announced_sha256(&format!("http://127.0.0.1:{port}/m.gguf"))
            .unwrap();
        server.join().unwrap();
        assert_eq!(got, Some(h));
    }
}
