//! First-run download of the speech models: the installer stays small and the
//! app fetches what it needs (Silero VAD, the recognition model), once, with
//! progress shown in the UI. Translation needs no download: it is done by the
//! LLM the user connects. Every file is checked by its exact published size
//! and SHA-256; an interrupted download resumes where it stopped.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use serde::Serialize;

use crate::speech::models;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileSpec {
    pub label: String,
    pub url: String,
    pub path: PathBuf,
    pub bytes: u64,
    pub sha256: Option<&'static str>,
}

fn complete(spec: &FileSpec) -> bool {
    std::fs::metadata(&spec.path).is_ok_and(|m| m.len() == spec.bytes)
}

/// Files the app needs and does not have yet: the VAD and the recognition model.
pub fn missing(dir: &Path, model: &str) -> Vec<FileSpec> {
    missing_all(dir, &[model])
}

/// Same, for several models of the catalog.
pub fn missing_all(dir: &Path, model_ids: &[&str]) -> Vec<FileSpec> {
    let mut all = vec![FileSpec {
        label: "Détection de parole (Silero)".into(),
        url: models::VAD_URL.into(),
        path: dir.join(models::VAD_FILE),
        bytes: models::VAD_BYTES,
        sha256: Some(models::VAD_SHA256),
    }];
    let mut seen = Vec::new();
    for spec in model_ids.iter().map(|id| models::spec_or_default(id)) {
        if seen.contains(&spec.id) {
            continue;
        }
        seen.push(spec.id);
        for f in spec.files {
            all.push(FileSpec {
                label: format!("Reconnaissance vocale ({}) · {}", spec.label, f.name),
                url: spec.url(f),
                path: dir.join(spec.dir).join(f.name),
                bytes: f.bytes,
                sha256: Some(f.sha256),
            });
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
}

pub struct HttpFetcher;

impl Fetcher for HttpFetcher {
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

/// Clears `running` when a download ends, normally or by a panic (which
/// is then reported as an error).
struct RunningGuard<'a>(&'a Downloader);

impl Drop for RunningGuard<'_> {
    fn drop(&mut self) {
        if std::thread::panicking() {
            let st = {
                let mut s = self.0.status.lock().unwrap_or_else(|e| e.into_inner());
                s.state = DownloadState::Error;
                s.current = None;
                s.error = Some("téléchargement interrompu (erreur interne)".into());
                s.clone()
            };
            self.0.events.progress(&st);
        }
        self.0.running.store(false, Ordering::SeqCst);
    }
}

pub struct Downloader {
    fetcher: Arc<dyn Fetcher>,
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
            }),
            running: AtomicBool::new(false),
            cancel: AtomicBool::new(false),
        })
    }

    /// Current state, with what is still missing for this configuration.
    pub fn status(&self, dir: &Path, model: &str) -> DownloadStatus {
        let todo = missing(dir, model);
        let mut st = self.status.lock().unwrap().clone();
        if !self.running.load(Ordering::SeqCst) {
            st.missing_files = todo.len();
            st.missing_bytes = todo.iter().map(|f| f.bytes).sum();
        }
        st
    }

    /// Starts downloading what is missing, on a background thread. Returns
    /// false when a download already runs or nothing is missing.
    pub fn start(self: &Arc<Self>, dir: PathBuf, model: &str) -> bool {
        self.start_all(dir, &[model])
    }

    /// Downloads exactly these files (other catalogues: the AI voices).
    pub fn start_files(self: &Arc<Self>, files: Vec<FileSpec>) -> bool {
        if files.is_empty() || self.running.swap(true, Ordering::SeqCst) {
            return false;
        }
        self.cancel.store(false, Ordering::SeqCst);
        let me = self.clone();
        let spawned = std::thread::Builder::new()
            .name("files-download".into())
            .spawn(move || me.run(files));
        if spawned.is_err() {
            self.running.store(false, Ordering::SeqCst);
            return false;
        }
        true
    }

    /// The last published state, with `missing` as the files still to fetch.
    pub fn snapshot(&self, missing: &[FileSpec]) -> DownloadStatus {
        let mut st = self.status.lock().unwrap().clone();
        if !self.running.load(Ordering::SeqCst) {
            st.missing_files = missing.len();
            st.missing_bytes = missing.iter().map(|f| f.bytes).sum();
        }
        st
    }

    /// [`Downloader::start`] for several Whisper models.
    pub fn start_all(self: &Arc<Self>, dir: PathBuf, model_ids: &[&str]) -> bool {
        let files = missing_all(&dir, model_ids);
        if files.is_empty() || self.running.swap(true, Ordering::SeqCst) {
            return false;
        }
        self.cancel.store(false, Ordering::SeqCst);
        let me = self.clone();
        let spawned = std::thread::Builder::new()
            .name("models-download".into())
            .spawn(move || me.run(files));
        if spawned.is_err() {
            self.running.store(false, Ordering::SeqCst);
            return false;
        }
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
        // Even if this thread panics, a later download can start and the
        // interface does not stay on "Téléchargement…".
        let _running = RunningGuard(self);
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
            }
        });
        let mut result = Ok(());
        for (i, f) in files.iter().enumerate() {
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

    fn download(&self, f: &FileSpec) -> Result<(), String> {
        if let Some(parent) = f.path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("dossier des modèles : {e}"))?;
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
                if last.elapsed().as_millis() >= 200 {
                    last = std::time::Instant::now();
                    let n = chunk.len() as u64;
                    self.publish(|s| s.done_bytes += n);
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
        if let Some(expected) = f.sha256 {
            if sha256_of(&part)? != expected {
                let _ = std::fs::remove_file(&part);
                return Err(format!(
                    "{} : empreinte SHA-256 invalide, fichier supprimé",
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

fn sha256_of(path: &Path) -> Result<String, String> {
    use sha2::{Digest, Sha256};
    let mut file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = file.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            return Ok(format!("{:x}", h.finalize()));
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

    fn spec(dir: &Path, name: &str, body: &[u8], sha256: Option<&'static str>) -> FileSpec {
        FileSpec {
            label: name.into(),
            url: format!("https://x/{name}"),
            path: dir.join(name),
            bytes: body.len() as u64,
            sha256,
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
        let sha = {
            use sha2::{Digest, Sha256};
            format!("{:x}", Sha256::digest(&body))
        };
        let f = spec(&d, "a.bin", &body, Some(Box::leak(sha.into_boxed_str())));
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

    /// Panics on the first file it serves.
    struct Boom;
    impl Fetcher for Boom {
        fn fetch(
            &self,
            _: &str,
            _: u64,
            _: &mut dyn FnMut(&[u8]) -> Result<(), String>,
        ) -> Result<(), String> {
            panic!("bug in a fetcher");
        }
    }

    #[test]
    fn a_panic_during_a_download_does_not_block_the_next_one() {
        let d = dir("panic");
        let f = spec(&d, "p.bin", b"model", None);
        let (tx, rx) = channel();
        let dl = Downloader::new(Arc::new(Boom), Arc::new(Events(Mutex::new(tx))));
        dl.running.store(true, Ordering::SeqCst);
        let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| dl.run(vec![f])));
        assert!(caught.is_err());
        assert!(
            !dl.running.load(Ordering::SeqCst),
            "a new download can start"
        );
        let end = wait_end(&rx);
        assert_eq!(end.state, DownloadState::Error);
        assert_eq!(end.current, None);
    }

    #[test]
    fn a_wrong_checksum_deletes_the_file() {
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
        assert!(end.error.unwrap().contains("SHA-256"));
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
    fn the_starter_pack_is_the_vad_and_the_recognition_model() {
        let d = dir("pack");
        let spec = models::spec(models::DEFAULT_MODEL).unwrap();
        let all = missing(&d, models::DEFAULT_MODEL);
        assert_eq!(all.len(), 1 + spec.files.len());
        assert!(all.iter().all(|f| f.url.starts_with("https://")));
        assert!(all.iter().all(|f| f.sha256.is_some_and(|s| s.len() == 64)));
        assert!(all[1..]
            .iter()
            .all(|f| f.path.starts_with(d.join(spec.dir))));
        let mb: u64 = all.iter().map(|f| f.bytes).sum::<u64>() / 1_000_000;
        assert!((600..750).contains(&mb), "{mb} Mo");
        std::fs::write(
            d.join(models::VAD_FILE),
            vec![0u8; models::VAD_BYTES as usize],
        )
        .unwrap();
        assert_eq!(missing(&d, models::DEFAULT_MODEL).len(), spec.files.len());
        // A Whisper id saved by an older version: the current model, once.
        assert_eq!(
            missing_all(&d, &["small", "base", models::DEFAULT_MODEL]).len(),
            spec.files.len()
        );
    }
}
