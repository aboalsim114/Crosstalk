//! In-app update: the app reads the update feed (`latest.json`, written by
//! scripts/publish-dev-build.sh next to the installer), downloads the newer
//! installer, checks its exact size and SHA-256, then runs it silently
//! (`/S /LAUNCH`): it installs into the same folder, keeps the models and the
//! settings, and starts Crosstalk again.
//!
//! Only HTTPS URLs on GitHub hosts are accepted; nothing is sent besides the
//! requests themselves.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::downloads::Fetcher;
use crate::version::current_build;

/// Update feed (overridable with `CROSSTALK_UPDATE_URL`).
pub const FEED_URL: &str =
    "https://raw.githubusercontent.com/aboalsim114/Crosstalk/downloads/latest.json";
pub const EVENT: &str = "update://status";
const ALLOWED_HOSTS: &[&str] = &[
    "raw.githubusercontent.com",
    "github.com",
    "objects.githubusercontent.com",
];
const MAX_MANIFEST: usize = 64 * 1024;

pub fn feed_url() -> String {
    std::env::var("CROSSTALK_UPDATE_URL").unwrap_or_else(|_| FEED_URL.to_string())
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Asset {
    pub url: String,
    pub sha256: String,
    pub bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Manifest {
    pub build: u32,
    pub version: String,
    pub commit: String,
    pub date: String,
    #[serde(default)]
    pub notes: Vec<String>,
    /// What's new, written for the players, newest first: shown before the
    /// update and once after it (release-notes.json).
    #[serde(default)]
    pub news: Vec<NewsEntry>,
    pub setup: Asset,
}

/// One change players can notice, with the build that first shipped it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NewsEntry {
    pub build: u32,
    pub date: String,
    pub title: String,
    pub items: Vec<String>,
}

const MAX_NEWS: usize = 12;
const MAX_NEWS_ITEMS: usize = 6;
const MAX_NEWS_CHARS: usize = 240;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum UpdateState {
    Idle,
    Checking,
    UpToDate,
    Available,
    Downloading,
    Ready,
    Installing,
    Error,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateStatus {
    pub state: UpdateState,
    pub build: u32,
    pub version: String,
    pub commit: String,
    pub latest: Option<Manifest>,
    pub done_bytes: u64,
    pub total_bytes: u64,
    pub error: Option<String>,
}

pub trait UpdateEvents: Send + Sync {
    fn status(&self, status: &UpdateStatus);
}

/// `https://<allowed host>/…` only.
pub fn allowed_url(url: &str) -> bool {
    let Some(rest) = url.strip_prefix("https://") else {
        return false;
    };
    let host = rest.split(['/', '?', '#']).next().unwrap_or("");
    ALLOWED_HOSTS.contains(&host)
}

/// Parses and checks a feed: sane fields and an allowed installer URL.
pub fn parse_manifest(bytes: &[u8]) -> Result<Manifest, String> {
    let m: Manifest = serde_json::from_slice(bytes)
        .map_err(|e| format!("fichier de mise à jour illisible : {e}"))?;
    if !allowed_url(&m.setup.url) {
        return Err("adresse de téléchargement refusée".into());
    }
    if m.setup.sha256.len() != 64 || !m.setup.sha256.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err("empreinte SHA-256 invalide".into());
    }
    if m.setup.bytes == 0 || m.setup.bytes > 200_000_000 {
        return Err("taille de l'installateur invalide".into());
    }
    let mut m = m;
    m.news = clean_news(std::mem::take(&mut m.news));
    Ok(m)
}

/// Keeps the news short and readable: a bad entry never blocks the update.
fn clean_news(news: Vec<NewsEntry>) -> Vec<NewsEntry> {
    let fits = |t: &str| !t.trim().is_empty() && t.chars().count() <= MAX_NEWS_CHARS;
    news.into_iter()
        .filter(|n| fits(&n.title) && n.date.len() <= 20)
        .map(|mut n| {
            n.items.retain(|i| fits(i));
            n.items.truncate(MAX_NEWS_ITEMS);
            n
        })
        .filter(|n| !n.items.is_empty())
        .take(MAX_NEWS)
        .collect()
}

pub struct Updater {
    fetcher: Arc<dyn Fetcher>,
    events: Arc<dyn UpdateEvents>,
    status: Mutex<UpdateStatus>,
    busy: AtomicBool,
    cancel: AtomicBool,
    dir: PathBuf,
    feed: String,
    ready: Mutex<Option<PathBuf>>,
}

impl Updater {
    pub fn new(
        fetcher: Arc<dyn Fetcher>,
        events: Arc<dyn UpdateEvents>,
        dir: PathBuf,
        feed: String,
    ) -> Arc<Self> {
        Arc::new(Self {
            fetcher,
            events,
            status: Mutex::new(UpdateStatus {
                state: UpdateState::Idle,
                build: current_build(),
                version: env!("CARGO_PKG_VERSION").into(),
                commit: env!("CROSSTALK_COMMIT").into(),
                latest: None,
                done_bytes: 0,
                total_bytes: 0,
                error: None,
            }),
            busy: AtomicBool::new(false),
            cancel: AtomicBool::new(false),
            dir,
            feed,
            ready: Mutex::new(None),
        })
    }

    pub fn status(&self) -> UpdateStatus {
        self.status.lock().unwrap().clone()
    }

    fn publish(&self, f: impl FnOnce(&mut UpdateStatus)) {
        let st = {
            let mut s = self.status.lock().unwrap();
            f(&mut s);
            s.clone()
        };
        self.events.status(&st);
    }

    fn fail(&self, e: String) {
        self.publish(|s| {
            s.state = UpdateState::Error;
            s.error = Some(e);
        });
    }

    /// Reads the feed (blocking). `Available` when it offers a newer build.
    pub fn check(&self) -> UpdateStatus {
        if self.busy.swap(true, Ordering::SeqCst) {
            return self.status();
        }
        self.publish(|s| {
            s.state = UpdateState::Checking;
            s.error = None;
        });
        let mut body = Vec::new();
        let fetched = self.fetcher.fetch(&self.feed, 0, &mut |chunk| {
            body.extend_from_slice(chunk);
            if body.len() > MAX_MANIFEST {
                return Err("fichier de mise à jour trop gros".into());
            }
            Ok(())
        });
        match fetched.and_then(|()| parse_manifest(&body)) {
            Ok(m) => {
                let newer = m.build > current_build();
                self.publish(|s| {
                    s.state = if newer {
                        UpdateState::Available
                    } else {
                        UpdateState::UpToDate
                    };
                    s.total_bytes = m.setup.bytes;
                    s.latest = Some(m);
                });
            }
            Err(e) => self.fail(format!("recherche impossible : {e}")),
        }
        self.busy.store(false, Ordering::SeqCst);
        self.status()
    }

    /// Downloads and checks the offered installer on a background thread.
    pub fn download(self: &Arc<Self>) -> bool {
        let Some(m) = self.status().latest.filter(|m| m.build > current_build()) else {
            return false;
        };
        if self.busy.swap(true, Ordering::SeqCst) {
            return false;
        }
        self.cancel.store(false, Ordering::SeqCst);
        let me = self.clone();
        std::thread::Builder::new()
            .name("update-download".into())
            .spawn(move || {
                match me.fetch_installer(&m) {
                    Ok(path) => {
                        *me.ready.lock().unwrap() = Some(path);
                        me.publish(|s| s.state = UpdateState::Ready);
                    }
                    Err(e) if me.cancel.load(Ordering::SeqCst) => {
                        let _ = e;
                        me.publish(|s| s.state = UpdateState::Available);
                    }
                    Err(e) => me.fail(e),
                }
                me.busy.store(false, Ordering::SeqCst);
            })
            .expect("update thread");
        true
    }

    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::SeqCst);
    }

    fn fetch_installer(&self, m: &Manifest) -> Result<PathBuf, String> {
        std::fs::create_dir_all(&self.dir).map_err(|e| format!("{} : {e}", self.dir.display()))?;
        let path = self.dir.join(format!("Crosstalk-setup-{}.exe", m.build));
        if verify(&path, &m.setup).is_ok() {
            return Ok(path); // already downloaded
        }
        let part = path.with_extension("exe.part");
        let _ = std::fs::remove_file(&part);
        self.publish(|s| {
            s.state = UpdateState::Downloading;
            s.done_bytes = 0;
            s.total_bytes = m.setup.bytes;
            s.error = None;
        });
        let mut out =
            std::fs::File::create(&part).map_err(|e| format!("écriture impossible : {e}"))?;
        let mut done = 0u64;
        let mut last = std::time::Instant::now();
        let r = self.fetcher.fetch(&m.setup.url, 0, &mut |chunk| {
            use std::io::Write;
            if self.cancel.load(Ordering::SeqCst) {
                return Err("annulé".into());
            }
            done += chunk.len() as u64;
            if done > m.setup.bytes {
                return Err("fichier plus grand que prévu".into());
            }
            out.write_all(chunk)
                .map_err(|e| format!("écriture impossible : {e}"))?;
            if last.elapsed().as_millis() >= 200 {
                last = std::time::Instant::now();
                self.publish(|s| s.done_bytes = done);
            }
            Ok(())
        });
        drop(out);
        if let Err(e) = r {
            let _ = std::fs::remove_file(&part);
            return Err(format!("téléchargement interrompu : {e}"));
        }
        if let Err(e) = verify(&part, &m.setup) {
            let _ = std::fs::remove_file(&part);
            return Err(e);
        }
        std::fs::rename(&part, &path).map_err(|e| format!("installation impossible : {e}"))?;
        self.publish(|s| s.done_bytes = m.setup.bytes);
        Ok(path)
    }

    /// The verified installer, ready to run.
    pub fn ready_installer(&self) -> Option<PathBuf> {
        self.ready.lock().unwrap().clone()
    }

    pub fn mark_installing(&self) {
        self.publish(|s| s.state = UpdateState::Installing);
    }
}

/// Exact size and SHA-256 of a downloaded installer.
pub fn verify(path: &Path, asset: &Asset) -> Result<(), String> {
    let mut f = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let len = f.metadata().map_err(|e| e.to_string())?.len();
    if len != asset.bytes {
        return Err(format!(
            "taille inattendue ({len} au lieu de {} octets)",
            asset.bytes
        ));
    }
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = f.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    let got = format!("{:x}", h.finalize());
    if !got.eq_ignore_ascii_case(&asset.sha256) {
        return Err("empreinte SHA-256 invalide : installateur refusé".into());
    }
    Ok(())
}

// ---------- Tauri ----------

pub struct TauriUpdateEvents(pub tauri::AppHandle);

impl UpdateEvents for TauriUpdateEvents {
    fn status(&self, status: &UpdateStatus) {
        use tauri::Emitter;
        let _ = self.0.emit(EVENT, status);
    }
}

#[tauri::command]
pub fn update_status(u: tauri::State<'_, Arc<Updater>>) -> UpdateStatus {
    u.status()
}

/// Reads the feed off the UI thread.
#[tauri::command]
pub async fn update_check(u: tauri::State<'_, Arc<Updater>>) -> Result<UpdateStatus, String> {
    let u = u.inner().clone();
    tauri::async_runtime::spawn_blocking(move || u.check())
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn update_download(u: tauri::State<'_, Arc<Updater>>) -> bool {
    u.download()
}

#[tauri::command]
pub fn update_cancel(u: tauri::State<'_, Arc<Updater>>) {
    u.cancel();
}

/// Runs the verified installer silently (same folder, models and settings
/// kept, Crosstalk restarted at the end), then quits so its files can be replaced.
#[tauri::command]
pub fn update_install(
    app: tauri::AppHandle,
    u: tauri::State<'_, Arc<Updater>>,
) -> Result<(), String> {
    let path = u.ready_installer().ok_or("aucune mise à jour prête")?;
    if let Some(m) = u.status().latest {
        verify(&path, &m.setup)?;
    }
    std::process::Command::new(&path)
        .args(["/S", "/LAUNCH"])
        .spawn()
        .map_err(|e| format!("lancement de l'installateur impossible : {e}"))?;
    u.mark_installing();
    app.exit(0);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Serve(Vec<(String, Vec<u8>)>);
    impl Fetcher for Serve {
        fn fetch(
            &self,
            url: &str,
            _: u64,
            sink: &mut dyn FnMut(&[u8]) -> Result<(), String>,
        ) -> Result<(), String> {
            let (_, body) = self
                .0
                .iter()
                .find(|(u, _)| u == url)
                .ok_or("le serveur a répondu 404")?;
            for c in body.chunks(1000) {
                sink(c)?;
            }
            Ok(())
        }
    }
    struct Quiet;
    impl UpdateEvents for Quiet {
        fn status(&self, _: &UpdateStatus) {}
    }

    const FEED: &str = "https://raw.githubusercontent.com/o/r/downloads/latest.json";
    const SETUP: &str = "https://raw.githubusercontent.com/o/r/downloads/Crosstalk-setup.exe";

    fn manifest(build: u32, body: &[u8], sha: Option<String>) -> Vec<u8> {
        let sha = sha.unwrap_or_else(|| format!("{:x}", Sha256::digest(body)));
        serde_json::to_vec(&serde_json::json!({
            "build": build, "version": "0.2.0", "commit": "abc1234", "date": "2026-10-13 00:00 UTC",
            "notes": ["fix: x"], "setup": {"url": SETUP, "sha256": sha, "bytes": body.len()}
        }))
        .unwrap()
    }

    fn updater(name: &str, files: Vec<(String, Vec<u8>)>) -> Arc<Updater> {
        let dir =
            std::env::temp_dir().join(format!("crosstalk-update-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        Updater::new(Arc::new(Serve(files)), Arc::new(Quiet), dir, FEED.into())
    }

    fn wait(u: &Updater, state: UpdateState) {
        let end = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while u.status().state != state {
            assert!(std::time::Instant::now() < end, "{:?}", u.status());
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }

    #[test]
    fn news_for_the_players_is_read_and_kept_short() {
        let long = "x".repeat(300);
        let feed = serde_json::to_vec(&serde_json::json!({
            "build": 280, "version": "0.4.1", "commit": "abc1234", "date": "2026-10-16 00:00 UTC",
            "news": [
                {"build": 280, "date": "2026-10-16", "title": "Nouveautés", "items": ["Une phrase.", long, ""]},
                {"build": 276, "date": "2026-10-16", "title": long, "items": ["Titre trop long : ignoré."]},
                {"build": 272, "date": "2026-10-16", "title": "Vide", "items": [long]}
            ],
            "setup": {"url": SETUP, "sha256": "a".repeat(64), "bytes": 10}
        }))
        .unwrap();
        let m = parse_manifest(&feed).unwrap();
        assert_eq!(m.news.len(), 1);
        assert_eq!(m.news[0].items, vec!["Une phrase."]);
        // Feeds without news (older publishes) still parse.
        let old = parse_manifest(&manifest(270, b"exe", None)).unwrap();
        assert!(old.news.is_empty());
    }

    #[test]
    fn a_newer_build_is_downloaded_and_verified() {
        let exe = vec![7u8; 5000];
        let u = updater(
            "ok",
            vec![
                (FEED.into(), manifest(current_build() + 1, &exe, None)),
                (SETUP.into(), exe.clone()),
            ],
        );
        assert_eq!(u.check().state, UpdateState::Available);
        assert!(u.download());
        wait(&u, UpdateState::Ready);
        let path = u.ready_installer().unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), exe);
        std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }

    #[test]
    fn same_or_older_builds_are_up_to_date() {
        let exe = vec![1u8; 10];
        let u = updater(
            "same",
            vec![(FEED.into(), manifest(current_build(), &exe, None))],
        );
        assert_eq!(u.check().state, UpdateState::UpToDate);
        assert!(!u.download());
    }

    #[test]
    fn a_wrong_fingerprint_is_refused_and_nothing_kept() {
        let exe = vec![7u8; 5000];
        let u = updater(
            "bad",
            vec![
                (
                    FEED.into(),
                    manifest(current_build() + 1, &exe, Some("0".repeat(64))),
                ),
                (SETUP.into(), exe),
            ],
        );
        assert_eq!(u.check().state, UpdateState::Available);
        assert!(u.download());
        wait(&u, UpdateState::Error);
        assert!(u.status().error.unwrap().contains("SHA-256"));
        assert!(u.ready_installer().is_none());
    }

    #[test]
    fn feeds_and_urls_are_checked() {
        assert!(allowed_url("https://raw.githubusercontent.com/a/b/c.exe"));
        assert!(allowed_url("https://github.com/a/b/raw/downloads/x.exe"));
        assert!(!allowed_url("http://raw.githubusercontent.com/a"));
        assert!(!allowed_url(
            "https://raw.githubusercontent.com.evil.example/a"
        ));
        assert!(!allowed_url("https://evil.example/github.com/a"));
        let bad = br#"{"build":2,"version":"0","commit":"x","date":"d","setup":{"url":"https://evil.example/s.exe","sha256":"00","bytes":1}}"#;
        assert!(parse_manifest(bad).unwrap_err().contains("refusée"));
        let u = updater("down", vec![]);
        assert_eq!(u.check().state, UpdateState::Error);
    }
}
