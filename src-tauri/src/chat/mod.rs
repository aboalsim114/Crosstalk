//! Real CS2 text chat (cahier des charges §4.1, §4.2), strictly external: CS2
//! writes its console to `console.log` when launched with `-condebug`, and
//! Crosstalk reads that file. No injection, no memory access, no network
//! interception; Steam's files are only read.
//!
//! ```text
//! steam (detection, -condebug) → tail (console.log) → parser (rules, dedup)
//!   → own messages filtered → lang (ru / en / fr / ?) → chat://message
//!                                                     → translation (ru, en)
//! ```
//!
//! The chat is followed only while a session runs (`chat_start` / `chat_stop`)
//! and nothing is stored.

pub mod lang;
pub mod parser;
pub mod steam;
pub mod tail;
pub mod vdf;

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use parser::{Channel, Dedup, Rules};
use steam::Cs2Setup;
use tail::{TailEvent, Tailer};

pub const MESSAGE_EVENT: &str = "chat://message";
pub const STATUS_EVENT: &str = "chat://status";
const POLL: Duration = Duration::from_millis(250);
/// Chat ids live apart from speech utterance ids (both go to the translator).
const FIRST_ID: u64 = 1 << 40;

#[derive(Debug, Clone, Default, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ChatConfig {
    /// CS2 folder or `console.log` given by the user when detection fails (F-04).
    pub manual_path: Option<String>,
    /// The player's own name, to leave out their messages (F-13).
    pub own_name: Option<String>,
    pub ignore_own: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatMessage {
    pub id: u64,
    pub channel: Channel,
    pub sender: String,
    pub message: String,
    pub location: Option<String>,
    pub dead: bool,
    /// `ru`, `en`, `fr`, another code, or `None` when the text gives no clue.
    pub lang: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ChatState {
    /// Not following (no session).
    Idle,
    /// Following, but `console.log` does not exist yet (CS2 never launched with -condebug).
    Waiting,
    Following,
    /// CS2 could not be found: a manual path is needed.
    NotFound,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatStatus {
    pub state: ChatState,
    pub setup: Cs2Setup,
    pub config: ChatConfig,
    pub rules_version: String,
    pub rules_file: Option<String>,
    pub rules_warning: Option<String>,
    /// Bytes of console.log already read.
    pub offset: u64,
    pub messages: u64,
    pub own_ignored: u64,
    pub duplicates: u64,
    /// Lines that look like chat but match no rule (game format changed?).
    pub unrecognized: u64,
}

pub trait ChatEvents: Send + Sync {
    fn status(&self, status: &ChatStatus);
    fn message(&self, message: &ChatMessage);
}

/// `%APPDATA%\Crosstalk\chat-rules.json`.
pub fn user_rules_file() -> Option<PathBuf> {
    std::env::var_os("APPDATA").map(|d| PathBuf::from(d).join("Crosstalk").join("chat-rules.json"))
}

struct Inner {
    events: Arc<dyn ChatEvents>,
    config: Mutex<ChatConfig>,
    status: Mutex<ChatStatus>,
    rules: Mutex<Rules>,
    rules_file: Option<PathBuf>,
    running: AtomicBool,
    /// Bumped at each start/stop: an old polling thread exits.
    generation: AtomicU64,
    next_id: AtomicU64,
}

pub struct ChatService {
    inner: Arc<Inner>,
}

impl Inner {
    fn publish(&self, f: impl FnOnce(&mut ChatStatus)) {
        let st = {
            let mut s = self.status.lock().unwrap();
            f(&mut s);
            s.clone()
        };
        self.events.status(&st);
    }

    fn detect(&self) -> Cs2Setup {
        let manual = self
            .config
            .lock()
            .unwrap()
            .manual_path
            .clone()
            .filter(|p| !p.trim().is_empty());
        steam::detect(manual.as_deref().map(Path::new))
    }
}

impl ChatService {
    pub fn new(events: Arc<dyn ChatEvents>, rules_file: Option<PathBuf>) -> Arc<Self> {
        let (rules, warning) = Rules::load(rules_file.as_deref());
        let config = ChatConfig {
            ignore_own: true,
            ..Default::default()
        };
        let status = ChatStatus {
            state: ChatState::Idle,
            setup: steam::detect(None),
            config: config.clone(),
            rules_version: rules.version.clone(),
            rules_file: rules.source.as_ref().map(|p| p.display().to_string()),
            rules_warning: warning,
            offset: 0,
            messages: 0,
            own_ignored: 0,
            duplicates: 0,
            unrecognized: 0,
        };
        Arc::new(Self {
            inner: Arc::new(Inner {
                events,
                config: Mutex::new(config),
                status: Mutex::new(status),
                rules: Mutex::new(rules),
                rules_file,
                running: AtomicBool::new(false),
                generation: AtomicU64::new(0),
                next_id: AtomicU64::new(FIRST_ID),
            }),
        })
    }

    pub fn status(&self) -> ChatStatus {
        self.inner.status.lock().unwrap().clone()
    }

    /// New settings; detection runs again (and the tail restarts if running).
    pub fn configure(&self, config: ChatConfig) -> ChatStatus {
        *self.inner.config.lock().unwrap() = config.clone();
        let setup = self.inner.detect();
        self.inner.publish(|s| {
            s.config = config;
            s.setup = setup;
        });
        if self.inner.running.load(Ordering::SeqCst) {
            self.start();
        }
        self.status()
    }

    /// Detection again (CS2 installed, -condebug added) and the rules file reloaded.
    pub fn rescan(&self) -> ChatStatus {
        let (rules, warning) = Rules::load(self.inner.rules_file.as_deref());
        let version = rules.version.clone();
        let file = rules.source.as_ref().map(|p| p.display().to_string());
        *self.inner.rules.lock().unwrap() = rules;
        let setup = self.inner.detect();
        self.inner.publish(|s| {
            s.setup = setup;
            s.rules_version = version;
            s.rules_file = file;
            s.rules_warning = warning;
        });
        self.status()
    }

    /// Follows console.log from its current end, on a background thread.
    pub fn start(&self) {
        let generation = self.inner.generation.fetch_add(1, Ordering::SeqCst) + 1;
        self.inner.running.store(true, Ordering::SeqCst);
        let setup = self.inner.detect();
        let Some(log) = setup.console_log.clone() else {
            self.inner.publish(|s| {
                s.setup = setup;
                s.state = ChatState::NotFound;
            });
            return;
        };
        self.inner.publish(|s| {
            s.setup = setup;
            s.state = ChatState::Following;
            s.offset = 0;
        });
        let inner = self.inner.clone();
        std::thread::Builder::new()
            .name("chat-tail".into())
            .spawn(move || follow(&inner, Path::new(&log), generation))
            .expect("chat thread");
    }

    pub fn stop(&self) {
        self.inner.generation.fetch_add(1, Ordering::SeqCst);
        self.inner.running.store(false, Ordering::SeqCst);
        self.inner.publish(|s| s.state = ChatState::Idle);
    }
}

fn follow(inner: &Inner, log: &Path, generation: u64) {
    let mut tail = Tailer::new(log);
    let mut dedup = Dedup::default();
    let mut last_state = None;
    while inner.generation.load(Ordering::SeqCst) == generation {
        let (lines, event) = tail.poll();
        let state = if event == Some(TailEvent::Missing) {
            ChatState::Waiting
        } else {
            ChatState::Following
        };
        if last_state != Some(state) || !lines.is_empty() {
            last_state = Some(state);
            let offset = tail.offset();
            let still = inner.generation.load(Ordering::SeqCst) == generation;
            if still {
                inner.publish(|s| {
                    s.state = state;
                    s.offset = offset;
                    if state == ChatState::Following {
                        s.setup.console_log_exists = true;
                    }
                });
            }
        }
        for line in lines {
            handle_line(inner, &line, &mut dedup, generation);
        }
        std::thread::sleep(POLL);
    }
}

fn handle_line(inner: &Inner, line: &str, dedup: &mut Dedup, generation: u64) {
    let Some(chat) = inner.rules.lock().unwrap().parse_line(line) else {
        if let Some(shape) = parser::chat_shape(line) {
            let n = {
                let mut n = 0;
                inner.publish(|s| {
                    s.unrecognized += 1;
                    n = s.unrecognized;
                });
                n
            };
            if n <= 5 {
                crate::logs::warn(
                    "chat",
                    &format!("ligne de chat non reconnue, forme : {shape}"),
                );
            }
        }
        return;
    };
    if !dedup.fresh(&chat, Instant::now()) {
        inner.publish(|s| s.duplicates += 1);
        return;
    }
    let cfg = inner.config.lock().unwrap().clone();
    let own = cfg
        .own_name
        .as_deref()
        .map(str::trim)
        .filter(|n| !n.is_empty());
    if cfg.ignore_own && own.is_some_and(|n| n.eq_ignore_ascii_case(&chat.sender)) {
        inner.publish(|s| s.own_ignored += 1);
        return;
    }
    if inner.generation.load(Ordering::SeqCst) != generation {
        return;
    }
    let msg = ChatMessage {
        id: inner.next_id.fetch_add(1, Ordering::SeqCst),
        lang: lang::detect(&chat.message).map(str::to_string),
        channel: chat.channel,
        sender: chat.sender,
        message: chat.message,
        location: chat.location,
        dead: chat.dead,
    };
    inner.publish(|s| s.messages += 1);
    inner.events.message(&msg);
}

// ---------- Tauri ----------

pub struct TauriChatEvents(pub tauri::AppHandle);

impl ChatEvents for TauriChatEvents {
    fn status(&self, status: &ChatStatus) {
        use tauri::Emitter;
        let _ = self.0.emit(STATUS_EVENT, status);
    }
    fn message(&self, m: &ChatMessage) {
        use tauri::{Emitter, Manager};
        let _ = self.0.emit(MESSAGE_EVENT, m);
        // A chat sender is a known name for "Qui parle ?" (memory only).
        if let Some(s) = self.0.try_state::<Arc<crate::speaker::SpeakerService>>() {
            s.chat_sender(&m.sender);
        }
        // Only languages with a model are sent; French is shown as is (F-32).
        if let Some(lang @ ("ru" | "en")) = m.lang.as_deref() {
            if let Some(t) = self
                .0
                .try_state::<Arc<crate::translation::TranslationService>>()
            {
                t.submit(m.id, lang, &m.message);
            }
        }
    }
}

#[tauri::command]
pub fn chat_status(chat: tauri::State<'_, Arc<ChatService>>) -> ChatStatus {
    chat.status()
}

#[tauri::command]
pub fn chat_configure(chat: tauri::State<'_, Arc<ChatService>>, config: ChatConfig) -> ChatStatus {
    chat.configure(config)
}

#[tauri::command]
pub fn chat_rescan(chat: tauri::State<'_, Arc<ChatService>>) -> ChatStatus {
    chat.rescan()
}

#[tauri::command]
pub fn chat_start(chat: tauri::State<'_, Arc<ChatService>>) -> ChatStatus {
    chat.start();
    chat.status()
}

#[tauri::command]
pub fn chat_stop(chat: tauri::State<'_, Arc<ChatService>>) {
    chat.stop();
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::sync::mpsc::{channel, Receiver, Sender};

    struct Events(Mutex<Sender<ChatMessage>>);
    impl ChatEvents for Events {
        fn status(&self, _: &ChatStatus) {}
        fn message(&self, m: &ChatMessage) {
            let _ = self.0.lock().unwrap().send(m.clone());
        }
    }

    fn setup(name: &str) -> (Arc<ChatService>, Receiver<ChatMessage>, PathBuf) {
        let dir =
            std::env::temp_dir().join(format!("crosstalk-chat-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let log = dir.join("game").join("csgo").join("console.log");
        std::fs::create_dir_all(log.parent().unwrap()).unwrap();
        std::fs::write(&log, "[ALL] old: before the session\n").unwrap();
        let (tx, rx) = channel();
        let svc = ChatService::new(Arc::new(Events(Mutex::new(tx))), None);
        svc.configure(ChatConfig {
            manual_path: Some(dir.display().to_string()),
            own_name: Some("Me".into()),
            ignore_own: true,
        });
        (svc, rx, log)
    }

    fn write(log: &Path, s: &str) {
        std::fs::OpenOptions::new()
            .append(true)
            .open(log)
            .unwrap()
            .write_all(s.as_bytes())
            .unwrap();
    }

    fn wait_following(svc: &ChatService) {
        let end = Instant::now() + Duration::from_secs(5);
        while svc.status().offset == 0 {
            assert!(Instant::now() < end);
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    #[test]
    fn follows_new_messages_only_during_a_session() {
        let (svc, rx, log) = setup("follow");
        let st = svc.status();
        assert!(st.setup.manual && st.setup.console_log_exists, "{st:?}");
        svc.start();
        wait_following(&svc);
        write(&log, "10/12 21:00:01  [CT] Иван\u{200e}\u{fe6b}B Site: Двое на B\nDamage Given to \"x\"\n[ALL] Me\u{200e}: my own line\n[ALL] Bob\u{200e}: gg\n[ALL] Bob\u{200e}: gg\n");
        let a = rx.recv_timeout(Duration::from_secs(5)).unwrap();
        assert_eq!(
            (
                a.channel,
                a.sender.as_str(),
                a.message.as_str(),
                a.lang.as_deref()
            ),
            (Channel::Ct, "Иван", "Двое на B", Some("ru"))
        );
        assert!(a.id >= FIRST_ID);
        let b = rx.recv_timeout(Duration::from_secs(5)).unwrap();
        assert_eq!((b.sender.as_str(), b.lang.as_deref()), ("Bob", None));
        assert!(
            rx.recv_timeout(Duration::from_millis(600)).is_err(),
            "duplicate or own line emitted"
        );
        let st = svc.status();
        assert_eq!((st.messages, st.own_ignored, st.duplicates), (2, 1, 1));

        svc.stop();
        std::thread::sleep(Duration::from_millis(400));
        write(&log, "[ALL] Bob\u{200e}: after the session\n");
        assert!(rx.recv_timeout(Duration::from_millis(600)).is_err());
        assert_eq!(svc.status().state, ChatState::Idle);
        std::fs::remove_dir_all(log.parent().unwrap().parent().unwrap().parent().unwrap()).unwrap();
    }

    #[test]
    fn a_missing_console_log_is_waited_for() {
        let (svc, rx, log) = setup("missing");
        std::fs::remove_file(&log).unwrap();
        svc.start();
        let end = Instant::now() + Duration::from_secs(5);
        while svc.status().state != ChatState::Waiting {
            assert!(Instant::now() < end);
            std::thread::sleep(Duration::from_millis(20));
        }
        std::fs::write(&log, "[T] Ann\u{200e}: they are going B\n").unwrap();
        let m = rx.recv_timeout(Duration::from_secs(5)).unwrap();
        assert_eq!(
            (m.message.as_str(), m.lang.as_deref()),
            ("they are going B", Some("en"))
        );
        svc.stop();
        std::fs::remove_dir_all(log.parent().unwrap().parent().unwrap().parent().unwrap()).unwrap();
    }
}
