//! Diagnostic journal (cahier des charges §5, §6.3): what the services do, in
//! `%LOCALAPPDATA%\Crosstalk\logs\crosstalk-<date>.log`, one file per day, kept
//! 7 days. It never holds what players say or write, nor their names: only the
//! state of the services (capture, engines, chat reader, updates) and errors.
//! The Windows user name is masked in paths. The Diagnostic screen shows the
//! latest lines and exports a report the user can attach to a bug report.

use std::collections::{HashMap, VecDeque};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{sync_channel, Receiver, SyncSender, TrySendError};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::history::{now_ms, utc};

pub const KEEP_DAYS: u64 = 7;
const RECENT: usize = 400;
const MAX_MESSAGE: usize = 500;
const DAY_MS: u64 = 24 * 3600 * 1000;
/// Lines waiting for the disk; beyond that (disk stalled) they are counted
/// and the count is written once the disk keeps up again.
const WRITE_QUEUE: usize = 1024;
const ISSUES_URL: &str = "https://github.com/aboalsim114/Crosstalk/issues/new";

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Line {
    /// Unix time, milliseconds.
    pub ts: u64,
    pub level: &'static str,
    pub source: String,
    pub message: String,
}

impl Line {
    fn text(&self) -> String {
        format!(
            "{} {:<5} {}: {}",
            utc((self.ts / 1000) as i64),
            self.level,
            self.source,
            self.message
        )
    }
}

/// Disk writes go through the `journal` thread: the threads that log (VAD,
/// Whisper, capture) never wait for the disk, nor hold a lock during it.
pub struct Logger {
    dir: Option<PathBuf>,
    inner: Mutex<Inner>,
    writer: Option<SyncSender<Job>>,
    /// Lines not written because the queue was full.
    dropped: AtomicU64,
}

enum Job {
    Line {
        path: PathBuf,
        text: String,
    },
    /// Answers once every line queued before it is on disk.
    Flush(SyncSender<()>),
}

fn write_loop(dir: PathBuf, jobs: Receiver<Job>) {
    for job in jobs {
        match job {
            Job::Line { path, text } => {
                let ok = std::fs::create_dir_all(&dir).is_ok()
                    && std::fs::OpenOptions::new()
                        .create(true)
                        .append(true)
                        .open(&path)
                        .and_then(|mut f| writeln!(f, "{text}"))
                        .is_ok();
                if !ok {
                    eprintln!("crosstalk: journal non écrit ({})", path.display());
                }
            }
            Job::Flush(done) => {
                let _ = done.send(());
            }
        }
    }
}

#[derive(Default)]
struct Inner {
    recent: VecDeque<Line>,
    /// Last line logged per status key: a status is logged when it changes.
    last: HashMap<String, String>,
}

impl Logger {
    /// `dir: None` keeps the lines in memory only (tests, unwritable folder).
    pub fn new(dir: Option<PathBuf>) -> Self {
        let writer = dir.clone().and_then(|d| {
            let (tx, rx) = sync_channel(WRITE_QUEUE);
            std::thread::Builder::new()
                .name("journal".into())
                .spawn(move || write_loop(d, rx))
                .ok()
                .map(|_| tx)
        });
        Self {
            dir,
            inner: Mutex::new(Inner::default()),
            writer,
            dropped: AtomicU64::new(0),
        }
    }

    /// Waits (at most `timeout`) until the lines logged so far are on disk.
    pub fn flush(&self, timeout: Duration) -> bool {
        let Some(writer) = &self.writer else {
            return true;
        };
        let (tx, rx) = sync_channel(1);
        writer.send(Job::Flush(tx)).is_ok() && rx.recv_timeout(timeout).is_ok()
    }

    fn write(&self, ts: u64, text: String) {
        let (Some(dir), Some(writer)) = (&self.dir, &self.writer) else {
            return;
        };
        let path = dir.join(file_name(ts));
        let lost = self.dropped.swap(0, Ordering::Relaxed);
        if lost > 0 {
            let note = format!(
                "{} WARN  journal: {lost} ligne(s) non écrite(s), disque trop lent",
                utc((ts / 1000) as i64)
            );
            if writer
                .try_send(Job::Line {
                    path: path.clone(),
                    text: note,
                })
                .is_err()
            {
                self.dropped.fetch_add(lost, Ordering::Relaxed);
            }
        }
        if let Err(TrySendError::Full(_)) = writer.try_send(Job::Line { path, text }) {
            self.dropped.fetch_add(1, Ordering::Relaxed);
        }
    }

    pub fn log(&self, level: &'static str, source: &str, message: &str) {
        self.log_at(now_ms(), level, source, message);
    }

    fn log_at(&self, ts: u64, level: &'static str, source: &str, message: &str) {
        let mut message = redact(message.trim());
        if message.len() > MAX_MESSAGE {
            let cut = (0..=MAX_MESSAGE)
                .rev()
                .find(|&i| message.is_char_boundary(i))
                .unwrap_or(0);
            message.truncate(cut);
            message.push('…');
        }
        let line = Line {
            ts,
            level,
            source: source.chars().take(32).collect(),
            message,
        };
        let text = line.text();
        if cfg!(debug_assertions) {
            eprintln!("{text}");
        }
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        // Queued under the lock: the file keeps the order of the lines.
        self.write(ts, text);
        if inner.recent.len() == RECENT {
            inner.recent.pop_front();
        }
        inner.recent.push_back(line);
    }

    /// Logs `message` only if it differs from the last one under `key`.
    pub fn changed(&self, key: &str, level: &'static str, source: &str, message: &str) {
        {
            let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            if inner.last.get(key).is_some_and(|m| m == message) {
                return;
            }
            inner.last.insert(key.to_string(), message.to_string());
        }
        self.log(level, source, message);
    }

    /// Latest lines, oldest first.
    pub fn recent(&self, limit: usize) -> Vec<Line> {
        let inner = self.inner.lock().unwrap();
        let skip = inner.recent.len().saturating_sub(limit);
        inner.recent.iter().skip(skip).cloned().collect()
    }

    pub fn dir(&self) -> Option<&Path> {
        self.dir.as_deref()
    }
}

static LOGGER: OnceLock<Logger> = OnceLock::new();

/// The process-wide journal (memory only until `init`).
pub fn global() -> &'static Logger {
    LOGGER.get_or_init(|| Logger::new(None))
}

/// Opens the journal folder, drops the files older than 7 days and records
/// panics. Called once at start-up.
pub fn init(dir: PathBuf) {
    purge(&dir, now_ms());
    let _ = LOGGER.set(Logger::new(Some(dir)));
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let at = info
            .location()
            .map(|l| format!(" ({}:{})", l.file(), l.line()))
            .unwrap_or_default();
        let what = info
            .payload()
            .downcast_ref::<&str>()
            .map(|s| s.to_string())
            .or_else(|| info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_default();
        error("panic", &format!("{what}{at}"));
        // The release build aborts right after: the line must reach the disk.
        global().flush(Duration::from_millis(500));
        previous(info);
    }));
}

pub fn default_dir() -> PathBuf {
    if let Some(d) = std::env::var_os("CROSSTALK_LOG_DIR") {
        return PathBuf::from(d);
    }
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("Crosstalk")
        .join("logs")
}

pub fn info(source: &str, message: &str) {
    global().log("INFO", source, message);
}

pub fn warn(source: &str, message: &str) {
    global().log("WARN", source, message);
}

pub fn error(source: &str, message: &str) {
    global().log("ERROR", source, message);
}

fn file_name(ts: u64) -> String {
    format!("crosstalk-{}.log", &utc((ts / 1000) as i64)[..10])
}

/// Deletes `crosstalk-<date>.log` files older than `KEEP_DAYS`.
pub fn purge(dir: &Path, now: u64) -> usize {
    let cutoff = file_name(now.saturating_sub(KEEP_DAYS * DAY_MS));
    let Ok(entries) = std::fs::read_dir(dir) else {
        return 0;
    };
    let mut removed = 0;
    for e in entries.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        // Same length and prefix: the dates compare as strings.
        let ours =
            name.starts_with("crosstalk-") && name.ends_with(".log") && name.len() == cutoff.len();
        if ours && name < cutoff && std::fs::remove_file(e.path()).is_ok() {
            removed += 1;
        }
    }
    removed
}

/// Masks the Windows user name (in profile paths and on its own).
pub fn redact(text: &str) -> String {
    static PROFILE: OnceLock<Regex> = OnceLock::new();
    let re = PROFILE.get_or_init(|| {
        Regex::new(
            r#"(?i)([a-z]:[\\/]+(?:users|documents and settings)[\\/]+|/home/|/Users/)[^\\/"'\n]+"#,
        )
        .unwrap()
    });
    let mut out = re.replace_all(text, "${1}[utilisateur]").into_owned();
    for var in ["USERNAME", "USER"] {
        if let Ok(name) = std::env::var(var) {
            if name.chars().count() >= 3 && out.contains(&name) {
                out = out.replace(&name, "[utilisateur]");
            }
        }
    }
    out
}

fn s<'a>(v: &'a Value, path: &[&str]) -> Option<&'a str> {
    path.iter()
        .try_fold(v, |v, k| v.get(k))
        .and_then(Value::as_str)
}

fn n(v: &Value, path: &[&str]) -> Option<u64> {
    path.iter()
        .try_fold(v, |v, k| v.get(k))
        .and_then(Value::as_u64)
}

/// One journal line per service status: `(key, level, source, message)`.
/// Counters that move all the time (bytes read, queue length) are left out so
/// that only real changes are logged.
pub fn summarize(event: &str, v: &Value) -> Vec<(String, &'static str, &'static str, String)> {
    let one = |level, source, msg: String| vec![(event.to_string(), level, source, msg)];
    match event {
        "cs2://status" => {
            if v.get("running").and_then(Value::as_bool) == Some(true) {
                one(
                    "INFO",
                    "cs2",
                    format!("cs2.exe lancé (PID {})", n(v, &["pid"]).unwrap_or(0)),
                )
            } else {
                one("INFO", "cs2", "cs2.exe non lancé".into())
            }
        }
        "audio://status" => {
            let state = s(v, &["state"]).unwrap_or("?");
            let (level, msg) = match state {
                "capturing" => {
                    let src = match s(v, &["source", "kind"]) {
                        Some("process") => format!(
                            "{} ciblé (PID {})",
                            s(v, &["source", "processName"]).unwrap_or("?"),
                            n(v, &["source", "pid"]).unwrap_or(0)
                        ),
                        _ => format!(
                            "tout le son du PC ({})",
                            s(v, &["source", "name"]).unwrap_or("?")
                        ),
                    };
                    let fmt = match (
                        n(v, &["format", "sampleRate"]),
                        n(v, &["format", "channels"]),
                    ) {
                        (Some(r), Some(c)) => format!(", {r} Hz, {c} canal(aux)"),
                        _ => String::new(),
                    };
                    match s(v, &["fallbackReason"]) {
                        Some(why) => ("WARN", format!("capture : {src}{fmt} — repli : {why}")),
                        None => ("INFO", format!("capture : {src}{fmt}")),
                    }
                }
                "error" => (
                    "WARN",
                    format!(
                        "capture impossible [{}] : {}",
                        s(v, &["error", "code"]).unwrap_or("?"),
                        s(v, &["error", "message"]).unwrap_or("")
                    ),
                ),
                "idle" => (
                    "INFO",
                    match s(v, &["stopReason"]) {
                        Some(r) => format!("capture arrêtée ({r})"),
                        None => "capture à l’arrêt".into(),
                    },
                ),
                other => ("INFO", format!("capture : {other}")),
            };
            one(level, "audio", msg)
        }
        "speech://status" => {
            let state = s(v, &["state"]).unwrap_or("?");
            let chosen = s(v, &["config", "model"]).unwrap_or("?");
            // Automatic fallback: whisper-base runs in place of the chosen model.
            let model = match s(v, &["fallbackFrom"]) {
                Some(from) => format!("base (remplace {from}, trop lent)"),
                None => chosen.to_string(),
            };
            let mut msg = format!(
                "whisper-{model} : {state} · {} · {} thread(s){}",
                s(v, &["backend"]).unwrap_or("?"),
                n(v, &["threads"]).unwrap_or(0),
                if v.get("vadReady").and_then(Value::as_bool) == Some(false) {
                    " · VAD absente"
                } else {
                    ""
                }
            );
            if let Some(ms) = n(v, &["loadMs"]) {
                msg.push_str(&format!(" · chargé en {ms} ms"));
            }
            if let Some(e) = s(v, &["error"]) {
                msg.push_str(&format!(" · {e}"));
            }
            let level = if matches!(state, "error" | "missing") {
                "WARN"
            } else {
                "INFO"
            };
            one(level, "asr", msg)
        }
        "translation://status" => {
            // The user's language (Langues), French by default.
            let target = s(v, &["config", "target"]).unwrap_or("fr");
            let ready = v.get("ready").and_then(Value::as_bool).unwrap_or(false);
            let (level, msg) = if ready {
                ("INFO", format!("traduction : LLM prêt → {target}"))
            } else {
                (
                    "WARN",
                    format!(
                        "traduction indisponible : {}",
                        s(v, &["problem"]).unwrap_or("?")
                    ),
                )
            };
            vec![(event.to_string(), level, "translation", msg)]
        }
        "chat://status" => {
            let state = s(v, &["state"]).unwrap_or("?");
            let condebug = s(v, &["setup", "condebug"]).unwrap_or("?");
            let log = match (
                s(v, &["setup", "consoleLog"]),
                v.pointer("/setup/consoleLogExists")
                    .and_then(Value::as_bool),
            ) {
                (None, _) => "console.log introuvable (CS2 non localisé)".to_string(),
                (Some(p), Some(true)) => p.to_string(),
                (Some(p), _) => format!("{p} (pas encore créé)"),
            };
            let mut msg = format!(
                "lecteur : {state} · -condebug {condebug} · règles {} · {log}",
                s(v, &["rulesVersion"]).unwrap_or("?")
            );
            if let Some(w) = s(v, &["rulesWarning"]) {
                msg.push_str(&format!(" · {w}"));
            }
            let level = if state == "notfound"
                || condebug == "absent"
                || v.get("rulesWarning").is_some_and(|w| !w.is_null())
            {
                "WARN"
            } else {
                "INFO"
            };
            one(level, "chat", msg)
        }
        "update://status" => {
            let state = s(v, &["state"]).unwrap_or("?");
            if state == "downloading" || state == "checking" {
                return one("INFO", "update", format!("mise à jour : {state}"));
            }
            let mut msg = format!(
                "mise à jour : {state} · build installé {}",
                n(v, &["build"]).unwrap_or(0)
            );
            if let Some(b) = n(v, &["latest", "build"]) {
                msg.push_str(&format!(" · dernier build {b}"));
            }
            if let Some(e) = s(v, &["error"]) {
                msg.push_str(&format!(" · {e}"));
            }
            one(
                if state == "error" { "WARN" } else { "INFO" },
                "update",
                msg,
            )
        }
        "models://download" => {
            let state = s(v, &["state"]).unwrap_or("?");
            if state == "downloading" {
                // One line per file, not per progress tick.
                let file = s(v, &["current"]).unwrap_or("?");
                return one("INFO", "models", format!("téléchargement de {file}"));
            }
            let mut msg = format!("téléchargement des modèles : {state}");
            if let Some(e) = s(v, &["error"]) {
                msg.push_str(&format!(" · {e}"));
            }
            one(
                if state == "error" { "WARN" } else { "INFO" },
                "models",
                msg,
            )
        }
        "gsi://status" => {
            let msg = if v.get("installed").and_then(Value::as_bool) != Some(true) {
                "carte en cours : désactivée".to_string()
            } else if let Some(e) = s(v, &["error"]) {
                format!("carte en cours : {e}")
            } else if let Some(m) = s(v, &["map"]) {
                format!(
                    "carte en cours : {m} · {} callouts",
                    n(v, &["callouts"]).unwrap_or(0)
                )
            } else if v.get("lastSeen").is_some_and(|t| !t.is_null()) {
                "carte en cours : CS2 connecté, dans les menus".to_string()
            } else {
                "carte en cours : en attente de CS2".to_string()
            };
            let level = if v.get("error").is_some_and(|e| !e.is_null()) {
                "WARN"
            } else {
                "INFO"
            };
            one(level, "gsi", msg)
        }
        _ => Vec::new(),
    }
}

pub const WATCHED: [&str; 8] = [
    "gsi://status",
    "cs2://status",
    "audio://status",
    "speech://status",
    "translation://status",
    "chat://status",
    "update://status",
    "models://download",
];

/// Journals every service status change (the events the window receives).
pub fn watch(app: &tauri::AppHandle) {
    use tauri::Listener;
    for event in WATCHED {
        app.listen_any(event, move |e| {
            if let Ok(v) = serde_json::from_str::<Value>(e.payload()) {
                for (key, level, source, msg) in summarize(event, &v) {
                    global().changed(&key, level, source, &msg);
                }
            }
        });
    }
}

/// A row of the Diagnostic screen's built-in tests.
#[derive(Debug, Clone, Deserialize)]
pub struct DiagRow {
    pub name: String,
    pub detail: String,
    pub state: String,
}

fn models_listing(dir: &Path) -> Vec<String> {
    let mut out = Vec::new();
    let mut walk = vec![(dir.to_path_buf(), 0)];
    while let Some((d, depth)) = walk.pop() {
        let Ok(entries) = std::fs::read_dir(&d) else {
            continue;
        };
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() && depth < 2 {
                walk.push((p, depth + 1));
            } else if let Ok(m) = e.metadata() {
                if m.is_file() {
                    let rel = p.strip_prefix(dir).unwrap_or(&p).display().to_string();
                    out.push(format!("{rel} · {:.1} Mo", m.len() as f64 / 1e6));
                }
            }
        }
    }
    out.sort();
    out
}

/// The report text: versions, system, models, built-in tests, latest journal
/// lines. Masked like the journal; no audio, no game text, no player name.
pub fn report_text(rows: &[DiagRow], lines: &[Line], models_dir: &Path, now: u64) -> String {
    let mut r = String::new();
    let mut add = |s: String| {
        r.push_str(&s);
        r.push('\n');
    };
    add("Rapport de diagnostic Crosstalk".into());
    add(format!("Généré le {}", utc((now / 1000) as i64)));
    add("Contenu : état de l’application uniquement. Ni audio, ni texte de partie, ni pseudo ; le nom d’utilisateur Windows est masqué.".into());
    add(String::new());
    add("== Version ==".into());
    add(format!(
        "Crosstalk {} · build {} · commit {}",
        env!("CARGO_PKG_VERSION"),
        crate::version::current_build(),
        env!("CROSSTALK_COMMIT")
    ));
    add(format!(
        "Windows : {} · {} processeur(s) logique(s)",
        crate::audio::platform::windows_build()
            .map(|b| format!("build {b}"))
            .unwrap_or_else(|| std::env::consts::OS.to_string()),
        std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(0)
    ));
    add(String::new());
    add(format!("== Modèles ({}) ==", models_dir.display()));
    let models = models_listing(models_dir);
    if models.is_empty() {
        add("aucun fichier".into());
    }
    for m in models {
        add(m);
    }
    add(String::new());
    add("== Tests intégrés ==".into());
    for row in rows {
        add(format!("[{}] {} — {}", row.state, row.name, row.detail));
    }
    add(String::new());
    add(format!("== Journal ({} dernières lignes) ==", lines.len()));
    for l in lines {
        add(l.text());
    }
    redact(&r)
}

fn percent_encode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}

/// New GitHub issue, pre-filled with the version (no personal data); the user
/// attaches the report file themselves.
pub fn issue_url(report: Option<&str>) -> String {
    let body = format!(
        "**Ce qui se passe :**\n\n\n**Ce que j’attendais :**\n\n\n**Étapes pour reproduire :**\n1. \n\n---\nCrosstalk {} · build {} · commit {} · Windows {}\n{}",
        env!("CARGO_PKG_VERSION"),
        crate::version::current_build(),
        env!("CROSSTALK_COMMIT"),
        crate::audio::platform::windows_build().map(|b| b.to_string()).unwrap_or_else(|| "?".into()),
        match report {
            Some(_) => "Rapport de diagnostic : glissez ici le fichier exporté depuis Diagnostic (Documents\\Crosstalk-diagnostic-….txt).",
            None => "Rapport de diagnostic : exportez-le depuis l’écran Diagnostic puis glissez-le ici.",
        }
    );
    format!(
        "{ISSUES_URL}?title={}&body={}",
        percent_encode("Problème : "),
        percent_encode(&body)
    )
}

/// Opens a folder or a web page with the Windows shell.
pub(crate) fn shell_open(target: &str) -> Result<(), String> {
    #[cfg(windows)]
    let cmd = std::process::Command::new("explorer").arg(target).spawn();
    #[cfg(not(windows))]
    let cmd = std::process::Command::new("xdg-open").arg(target).spawn();
    cmd.map(drop)
        .map_err(|e| format!("ouverture impossible : {e}"))
}

#[tauri::command]
pub fn logs_recent(limit: usize) -> Vec<Line> {
    global().recent(limit.min(RECENT))
}

/// Errors seen by the interface (uncaught exceptions, failed calls).
#[tauri::command]
pub fn logs_write(level: String, source: String, message: String) {
    let level = match level.as_str() {
        "ERROR" => "ERROR",
        "WARN" => "WARN",
        _ => "INFO",
    };
    global().log(
        level,
        &format!("ui/{}", source.chars().take(24).collect::<String>()),
        &message,
    );
}

#[tauri::command]
pub fn logs_open_folder() -> Result<String, String> {
    let dir = global()
        .dir()
        .map(Path::to_path_buf)
        .unwrap_or_else(default_dir);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    shell_open(&dir.display().to_string())?;
    Ok(dir.display().to_string())
}

#[tauri::command]
pub fn diag_report(rows: Vec<DiagRow>) -> Result<String, String> {
    let now = now_ms();
    let text = report_text(
        &rows,
        &global().recent(200),
        &crate::paths::models_dir(),
        now,
    );
    let path = crate::history::documents_path("Crosstalk-diagnostic", "txt", now);
    std::fs::write(&path, text).map_err(|e| format!("écriture impossible : {e}"))?;
    info("diag", "rapport de diagnostic exporté");
    Ok(path.display().to_string())
}

#[tauri::command]
pub fn diag_report_issue(exported: bool) -> Result<(), String> {
    shell_open(&issue_url(exported.then_some("")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const DAY: u64 = DAY_MS;
    // 2026-10-13 12:00:00 UTC
    const NOW: u64 = 1_791_892_800_000;

    #[test]
    fn writes_one_file_per_day_and_keeps_the_latest_lines() {
        let dir = tempdir("write");
        let log = Logger::new(Some(dir.clone()));
        log.log_at(NOW - DAY, "INFO", "audio", "hier");
        log.log_at(NOW, "WARN", "asr", "aujourd’hui");
        assert!(log.flush(Duration::from_secs(5)));
        let today = std::fs::read_to_string(dir.join("crosstalk-2026-10-13.log")).unwrap();
        assert_eq!(today, "2026-10-13 12:00:00 UTC WARN  asr: aujourd’hui\n");
        assert!(dir.join("crosstalk-2026-10-12.log").exists());
        let recent = log.recent(1);
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].message, "aujourd’hui");
        for i in 0..RECENT + 10 {
            log.log_at(NOW, "INFO", "x", &i.to_string());
        }
        assert_eq!(log.recent(usize::MAX).len(), RECENT);
    }

    #[test]
    fn a_burst_of_lines_is_written_or_counted_never_silently_lost() {
        let dir = tempdir("burst");
        let log = Logger::new(Some(dir.clone()));
        for i in 0..3000 {
            log.log_at(NOW, "INFO", "asr", &format!("ligne {i}"));
        }
        // Once the disk has caught up, the next line carries the count of
        // what it could not take.
        assert!(log.flush(Duration::from_secs(10)));
        log.log_at(NOW, "INFO", "asr", "fin");
        assert!(log.flush(Duration::from_secs(10)));
        let text = std::fs::read_to_string(dir.join("crosstalk-2026-10-13.log")).unwrap();
        let written: Vec<u32> = text
            .lines()
            .filter_map(|l| l.split("ligne ").nth(1)?.parse().ok())
            .collect();
        let lost: u64 = text
            .lines()
            .filter_map(|l| {
                l.split("journal: ")
                    .nth(1)?
                    .split(' ')
                    .next()?
                    .parse::<u64>()
                    .ok()
            })
            .sum();
        assert_eq!(written.len() as u64 + lost, 3000, "{lost} lost");
        assert!(written.windows(2).all(|w| w[0] < w[1]), "order kept");
        assert!(text.trim_end().ends_with("asr: fin"));
    }

    #[test]
    fn logs_a_status_only_when_it_changes() {
        let log = Logger::new(None);
        log.changed("k", "INFO", "a", "un");
        log.changed("k", "INFO", "a", "un");
        log.changed("k", "INFO", "a", "deux");
        log.changed("j", "INFO", "a", "deux");
        assert_eq!(log.recent(10).len(), 3);
    }

    #[test]
    fn purges_files_older_than_seven_days() {
        let dir = tempdir("purge");
        for name in [
            "crosstalk-2026-10-05.log",
            "crosstalk-2026-10-06.log",
            "crosstalk-2026-10-13.log",
            "notes.txt",
        ] {
            std::fs::write(dir.join(name), "x").unwrap();
        }
        assert_eq!(purge(&dir, NOW), 1);
        assert!(!dir.join("crosstalk-2026-10-05.log").exists());
        assert!(dir.join("crosstalk-2026-10-06.log").exists());
        assert!(dir.join("notes.txt").exists());
    }

    #[test]
    fn masks_the_user_name() {
        assert_eq!(
            redact(r"C:\Users\Jean Dupont\AppData\Local\Crosstalk"),
            r"C:\Users\[utilisateur]\AppData\Local\Crosstalk"
        );
        assert_eq!(redact("d:/users/bob/x.log"), "d:/users/[utilisateur]/x.log");
        assert_eq!(
            redact(r"D:\SteamLibrary\steamapps"),
            r"D:\SteamLibrary\steamapps"
        );
        let long = "é".repeat(MAX_MESSAGE);
        let log = Logger::new(None);
        log.log("INFO", "t", &long);
        assert!(log.recent(1)[0].message.ends_with('…'));
    }

    /// The journal reads the statuses by their serialised field names: built
    /// from the real types, so renaming a field breaks this test, not the
    /// journal silently (docs/architecture/audit.md, A18).
    #[test]
    fn summarizes_the_real_status_types() {
        use crate::audio::CaptureStatus;
        use crate::speech::{EngineState, SpeechConfig, SpeechStatus};
        use crate::translation::{TranslationConfig, TranslationStatus};

        let speech = SpeechStatus {
            state: EngineState::Ready,
            config: SpeechConfig {
                model: "small".into(),
                ..SpeechConfig::default()
            },
            model_path: None,
            vad_ready: false,
            models_dir: "models".into(),
            error: Some("lent".into()),
            load_ms: Some(900),
            backend: "CPU · AVX2".into(),
            threads: 4,
            fallback_from: None,
        };
        let line = &summarize("speech://status", &serde_json::to_value(&speech).unwrap())[0];
        assert_eq!(
            (line.1, line.3.as_str()),
            (
                "INFO",
                "whisper-small : ready · CPU · AVX2 · 4 thread(s) · VAD absente · chargé en 900 ms · lent"
            )
        );
        // Automatic fallback: the journal names the model that really runs.
        let fallback = SpeechStatus {
            fallback_from: Some("small".into()),
            error: None,
            vad_ready: true,
            ..speech
        };
        let line = &summarize("speech://status", &serde_json::to_value(&fallback).unwrap())[0];
        assert_eq!(
            line.3,
            "whisper-base (remplace small, trop lent) : ready · CPU · AVX2 · 4 thread(s) · chargé en 900 ms"
        );

        let translation = TranslationStatus {
            config: TranslationConfig::default(),
            ready: false,
            problem: Some("aucun moteur de traduction connecté".into()),
            queued: 0,
        };
        let line = &summarize(
            "translation://status",
            &serde_json::to_value(&translation).unwrap(),
        )[0];
        assert_eq!(
            (line.1, line.3.as_str()),
            (
                "WARN",
                "traduction indisponible : aucun moteur de traduction connecté"
            )
        );

        // A Russian user: the arrow points to their language.
        let into_russian = TranslationStatus {
            config: TranslationConfig {
                target: "ru".into(),
                ..TranslationConfig::default()
            },
            ready: true,
            problem: None,
            queued: 0,
        };
        let line = &summarize(
            "translation://status",
            &serde_json::to_value(&into_russian).unwrap(),
        )[0];
        assert_eq!(
            (line.1, line.3.as_str()),
            ("INFO", "traduction : LLM prêt → ru")
        );

        let idle = serde_json::to_value(CaptureStatus::idle()).unwrap();
        assert_eq!(summarize("audio://status", &idle)[0].3, "capture à l’arrêt");
    }

    #[test]
    fn summarizes_service_statuses() {
        let cap = json!({"state":"capturing","source":{"kind":"process","pid":42,"processName":"cs2.exe"},"format":{"sampleRate":48000,"channels":2},"fallbackReason":null,"error":null});
        assert_eq!(
            summarize("audio://status", &cap)[0].3,
            "capture : cs2.exe ciblé (PID 42), 48000 Hz, 2 canal(aux)"
        );
        let err = json!({"state":"error","error":{"code":"processLoopback","message":"refusé"}});
        let e = &summarize("audio://status", &err)[0];
        assert_eq!(
            (e.1, e.3.as_str()),
            ("WARN", "capture impossible [processLoopback] : refusé")
        );
        let sp = json!({"state":"ready","config":{"model":"small"},"backend":"CPU","threads":4,"vadReady":true,"loadMs":900,"error":null});
        assert_eq!(
            summarize("speech://status", &sp)[0].3,
            "whisper-small : ready · CPU · 4 thread(s) · chargé en 900 ms"
        );
        let tr = json!({"ready":false,"problem":"clé refusée par le fournisseur","queued":3});
        let t = summarize("translation://status", &tr);
        assert_eq!((t.len(), t[0].1), (1, "WARN"));
        let chat = json!({"state":"following","offset":123,"messages":9,"rulesVersion":"cs2-1","rulesWarning":null,
            "setup":{"condebug":"present","consoleLog":"D:\\cs2\\console.log","consoleLogExists":true}});
        let c = &summarize("chat://status", &chat)[0];
        assert_eq!(
            (c.1, c.3.as_str()),
            (
                "INFO",
                "lecteur : following · -condebug present · règles cs2-1 · D:\\cs2\\console.log"
            )
        );
        // Counters do not produce new lines.
        let chat2 = json!({"state":"following","offset":999,"messages":12,"rulesVersion":"cs2-1","rulesWarning":null,
            "setup":{"condebug":"present","consoleLog":"D:\\cs2\\console.log","consoleLogExists":true}});
        assert_eq!(summarize("chat://status", &chat2)[0].3, c.3);
        assert_eq!(
            summarize("cs2://status", &json!({"running":true,"pid":7}))[0].3,
            "cs2.exe lancé (PID 7)"
        );
        let up = json!({"state":"available","build":96,"latest":{"build":98},"error":null});
        assert_eq!(
            summarize("update://status", &up)[0].3,
            "mise à jour : available · build installé 96 · dernier build 98"
        );
        assert!(summarize("speech://transcript", &json!({"text":"secret"})).is_empty());
    }

    #[test]
    fn report_holds_versions_tests_and_journal_without_the_user_name() {
        let dir = tempdir("report");
        std::fs::create_dir_all(dir.join("opus-mt-ru-fr")).unwrap();
        std::fs::write(dir.join("ggml-small.bin"), vec![0u8; 2_000_000]).unwrap();
        std::fs::write(dir.join("opus-mt-ru-fr").join("config.json"), "{}").unwrap();
        let rows = vec![DiagRow {
            name: "Journal console.log".into(),
            detail: r"C:\Users\Alice\cs2\console.log".into(),
            state: "OK".into(),
        }];
        let lines = vec![Line {
            ts: NOW,
            level: "INFO",
            source: "asr".into(),
            message: "prêt".into(),
        }];
        let r = report_text(&rows, &lines, &dir, NOW);
        assert!(r.contains("build "));
        assert!(r.contains("ggml-small.bin · 2.0 Mo"));
        assert!(r.contains("config.json"));
        assert!(r.contains(r"[OK] Journal console.log — C:\Users\[utilisateur]\cs2\console.log"));
        assert!(r.contains("2026-10-13 12:00:00 UTC INFO  asr: prêt"));
        assert!(!r.contains("Alice"));
    }

    #[test]
    fn issue_link_is_github_and_encoded() {
        let u = issue_url(Some(""));
        assert!(u.starts_with(
            "https://github.com/aboalsim114/Crosstalk/issues/new?title=Probl%C3%A8me"
        ));
        assert!(!u.contains(' ') && !u.contains('\n'));
    }

    fn tempdir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("crosstalk-logs-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }
}
