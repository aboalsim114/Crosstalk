//! Map being played, from CS2's Game State Integration (A-06, F-36).
//!
//! Game State Integration is Valve's official interface: CS2 reads a
//! `gamestate_integration_*.cfg` file from its `cfg` folder at launch and POSTs
//! the game state as JSON to the address given there. Crosstalk asks only for
//! the map (`"map" "1"`): no player, position or round data. Nothing is read
//! from or written to the game's process. The file is written only when the
//! user clicks "Activer" and removed by "Retirer".
//!
//! The map's callouts are then given to Whisper (initial prompt) and kept as
//! spoken in translations.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const STATUS_EVENT: &str = "gsi://status";
pub const CFG_NAME: &str = "gamestate_integration_crosstalk.cfg";
/// Local port CS2 posts to (127.0.0.1 only).
pub const PORT: u16 = 31982;
const MAX_BODY: usize = 256 * 1024;
const CS2_APPID: u64 = 730;

#[derive(Debug, Clone, Deserialize)]
pub struct MapInfo {
    pub name: String,
    /// Callouts given to Whisper.
    pub prompt: Vec<String>,
    /// Callouts kept as spoken in translations.
    pub keep: Vec<String>,
}

/// Callouts of the known maps (`maps.json`).
pub fn maps() -> &'static std::collections::HashMap<String, MapInfo> {
    static MAPS: OnceLock<std::collections::HashMap<String, MapInfo>> = OnceLock::new();
    MAPS.get_or_init(|| {
        let v: serde_json::Map<String, Value> =
            serde_json::from_str(include_str!("maps.json")).expect("maps.json");
        v.into_iter()
            .filter(|(k, _)| !k.starts_with('_'))
            .filter_map(|(k, v)| Some((k, serde_json::from_value(v).ok()?)))
            .collect()
    })
}

/// `de_mirage` → `Mirage`; unknown maps keep their technical name.
pub fn pretty(map: &str) -> String {
    maps()
        .get(map)
        .map(|m| m.name.clone())
        .unwrap_or_else(|| map.to_string())
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GsiStatus {
    /// Our cfg file is in CS2's cfg folder.
    pub installed: bool,
    pub cfg_path: Option<String>,
    /// CS2's folder is unknown: nothing can be installed.
    pub cs2_found: bool,
    pub listening: bool,
    pub port: u16,
    pub error: Option<String>,
    /// Technical name (`de_mirage`) and display name.
    pub map: Option<String>,
    pub map_name: Option<String>,
    pub mode: Option<String>,
    /// Callouts known for this map (0: unknown map, general vocabulary only).
    pub callouts: usize,
    /// Unix ms of the last message from CS2.
    pub last_seen: Option<u64>,
}

/// What the rest of the app does with the map.
pub trait GsiEvents: Send + Sync {
    fn status(&self, status: &GsiStatus);
    /// The map changed (`None`: menu or CS2 closed).
    fn map(&self, map: Option<&str>);
}

pub fn cfg_dir(cs2_dir: &Path) -> PathBuf {
    cs2_dir.join("game").join("csgo").join("cfg")
}

/// The cfg file (VDF). Only the map is requested.
pub fn cfg_text(token: &str) -> String {
    format!(
        "\"Crosstalk\"\n{{\n  \"uri\" \"http://127.0.0.1:{PORT}/\"\n  \"timeout\" \"5.0\"\n  \"buffer\" \"0.5\"\n  \"throttle\" \"1.0\"\n  \"heartbeat\" \"30.0\"\n  \"auth\"\n  {{\n    \"token\" \"{token}\"\n  }}\n  \"data\"\n  {{\n    \"provider\" \"1\"\n    \"map\" \"1\"\n  }}\n}}\n"
    )
}

/// Token of an installed cfg file.
pub fn token_of(cfg: &str) -> Option<String> {
    let at = cfg.find("\"token\"")? + "\"token\"".len();
    let rest = &cfg[at..];
    let start = rest.find('"')? + 1;
    let end = start + rest[start..].find('"')?;
    Some(rest[start..end].to_string()).filter(|t| !t.is_empty())
}

fn new_token() -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(crate::history::now_ms().to_le_bytes());
    h.update(std::process::id().to_le_bytes());
    h.update(format!("{:?}", std::thread::current().id()));
    let local = 0u8;
    h.update(format!("{:p}", &local));
    h.finalize()[..12]
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// What a POST from CS2 says about the map: `Some(None)` in the menus,
/// `None` when the message is not for us (wrong token or game).
pub fn parse_state(body: &str, token: &str) -> Option<Option<(String, Option<String>)>> {
    let v: Value = serde_json::from_str(body).ok()?;
    if v.pointer("/auth/token").and_then(Value::as_str) != Some(token) {
        return None;
    }
    if let Some(app) = v.pointer("/provider/appid").and_then(Value::as_u64) {
        if app != CS2_APPID {
            return None;
        }
    }
    let name = v
        .pointer("/map/name")
        .and_then(Value::as_str)
        .map(|s| s.trim().to_ascii_lowercase())
        .filter(|s| !s.is_empty() && s.len() <= 64);
    let mode = v
        .pointer("/map/mode")
        .and_then(Value::as_str)
        .map(str::to_string);
    Some(name.map(|n| (n, mode)))
}

/// Reads one HTTP request (headers + body) from a local connection.
fn read_request(stream: &mut TcpStream) -> Option<String> {
    stream.set_read_timeout(Some(Duration::from_secs(2))).ok()?;
    let mut buf = Vec::with_capacity(4096);
    let mut chunk = [0u8; 4096];
    let header_end = loop {
        let n = stream.read(&mut chunk).ok()?;
        if n == 0 {
            return None;
        }
        buf.extend_from_slice(&chunk[..n]);
        if let Some(i) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            break i + 4;
        }
        if buf.len() > 16 * 1024 {
            return None;
        }
    };
    let head = String::from_utf8_lossy(&buf[..header_end]).to_ascii_lowercase();
    if !head.starts_with("post ") {
        return None;
    }
    let len: usize = head
        .lines()
        .find_map(|l| l.strip_prefix("content-length:"))
        .and_then(|v| v.trim().parse().ok())?;
    if len > MAX_BODY {
        return None;
    }
    while buf.len() < header_end + len {
        let n = stream.read(&mut chunk).ok()?;
        if n == 0 {
            return None;
        }
        buf.extend_from_slice(&chunk[..n]);
    }
    String::from_utf8(buf[header_end..header_end + len].to_vec()).ok()
}

pub struct GsiService {
    events: Arc<dyn GsiEvents>,
    status: Mutex<GsiStatus>,
    token: Mutex<Option<String>>,
    started: AtomicBool,
    port: u16,
}

impl GsiService {
    pub fn new(events: Arc<dyn GsiEvents>) -> Arc<Self> {
        Self::with_port(events, PORT)
    }

    pub fn with_port(events: Arc<dyn GsiEvents>, port: u16) -> Arc<Self> {
        Arc::new(Self {
            events,
            status: Mutex::new(GsiStatus {
                port,
                ..Default::default()
            }),
            token: Mutex::new(None),
            started: AtomicBool::new(false),
            port,
        })
    }

    pub fn status(&self) -> GsiStatus {
        self.status.lock().unwrap().clone()
    }

    fn publish(&self) -> GsiStatus {
        let st = self.status();
        self.events.status(&st);
        st
    }

    /// Looks for our cfg file in CS2's folder; listens if it is there.
    pub fn refresh(self: &Arc<Self>, cs2_dir: Option<&Path>) -> GsiStatus {
        let cfg = cs2_dir.map(|d| cfg_dir(d).join(CFG_NAME));
        let token = cfg
            .as_ref()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|t| token_of(&t));
        {
            let mut st = self.status.lock().unwrap();
            st.cs2_found = cs2_dir.is_some();
            st.cfg_path = cfg.map(|p| p.display().to_string());
            st.installed = token.is_some();
        }
        let installed = token.is_some();
        *self.token.lock().unwrap() = token;
        if installed {
            self.listen();
        }
        self.publish()
    }

    /// Writes the cfg file (CS2 reads it at its next launch).
    pub fn install(self: &Arc<Self>, cs2_dir: &Path) -> Result<GsiStatus, String> {
        let dir = cfg_dir(cs2_dir);
        if !dir.is_dir() {
            return Err(format!(
                "dossier cfg de CS2 introuvable : {}",
                dir.display()
            ));
        }
        let token = self.token.lock().unwrap().clone().unwrap_or_else(new_token);
        std::fs::write(dir.join(CFG_NAME), cfg_text(&token))
            .map_err(|e| format!("écriture impossible dans {} : {e}", dir.display()))?;
        Ok(self.refresh(Some(cs2_dir)))
    }

    /// Removes the cfg file; the map is forgotten.
    pub fn uninstall(self: &Arc<Self>, cs2_dir: &Path) -> Result<GsiStatus, String> {
        let path = cfg_dir(cs2_dir).join(CFG_NAME);
        match std::fs::remove_file(&path) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
                return Err(format!("suppression impossible : {e}"))
            }
            _ => {}
        }
        self.set_map(None);
        Ok(self.refresh(Some(cs2_dir)))
    }

    /// CS2 closed: no map any more.
    pub fn game_closed(&self) {
        self.set_map(None);
    }

    fn set_map(&self, map: Option<(String, Option<String>)>) {
        let changed = {
            let mut st = self.status.lock().unwrap();
            let name = map.as_ref().map(|m| m.0.clone());
            let changed = st.map != name;
            st.map_name = name.as_deref().map(pretty);
            st.callouts = name
                .as_deref()
                .and_then(|n| maps().get(n))
                .map_or(0, |m| m.prompt.len());
            st.map = name;
            st.mode = map.and_then(|m| m.1);
            changed
        };
        if changed {
            let map = self.status().map;
            self.events.map(map.as_deref());
            self.publish();
        }
    }

    /// One POST from CS2.
    pub fn on_message(&self, body: &str) -> bool {
        let Some(token) = self.token.lock().unwrap().clone() else {
            return false;
        };
        let Some(map) = parse_state(body, &token) else {
            return false;
        };
        let first = {
            let mut st = self.status.lock().unwrap();
            let first = st.last_seen.is_none();
            st.last_seen = Some(crate::history::now_ms());
            first
        };
        self.set_map(map);
        if first {
            self.publish();
        }
        true
    }

    /// Starts the local server once (127.0.0.1 only).
    fn listen(self: &Arc<Self>) {
        if self.started.swap(true, Ordering::SeqCst) {
            return;
        }
        match TcpListener::bind(("127.0.0.1", self.port)) {
            Ok(listener) => {
                {
                    let mut st = self.status.lock().unwrap();
                    st.listening = true;
                    st.error = None;
                }
                let me = self.clone();
                std::thread::Builder::new()
                    .name("gsi".into())
                    .spawn(move || {
                        for stream in listener.incoming() {
                            let Ok(mut stream) = stream else { continue };
                            let ok = read_request(&mut stream).is_some_and(|b| me.on_message(&b));
                            let reply = if ok {
                                "HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                            } else {
                                "HTTP/1.1 403 Forbidden\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                            };
                            let _ = stream.write_all(reply.as_bytes());
                        }
                    })
                    .expect("gsi thread");
            }
            Err(e) => {
                self.started.store(false, Ordering::SeqCst);
                let mut st = self.status.lock().unwrap();
                st.listening = false;
                st.error = Some(format!(
                    "port local {} indisponible ({e}) : une autre application l’utilise",
                    self.port
                ));
            }
        }
    }
}

pub struct TauriGsiEvents {
    pub app: tauri::AppHandle,
    pub speech: Arc<crate::speech::SpeechService>,
    pub translation: Arc<crate::translation::TranslationService>,
    pub reply: Arc<crate::reply::ReplyService>,
}

impl GsiEvents for TauriGsiEvents {
    fn status(&self, status: &GsiStatus) {
        use tauri::Emitter;
        let _ = self.app.emit(STATUS_EVENT, status);
    }

    fn map(&self, map: Option<&str>) {
        let info = map.and_then(|m| maps().get(m));
        self.speech
            .set_map_prompt(info.map(|i| i.prompt.clone()).unwrap_or_default());
        let keep = info.map(|i| i.keep.clone()).unwrap_or_default();
        self.reply.set_map_terms(keep.clone());
        self.translation.set_map_terms(keep);
    }
}

fn cs2_dir(chat: &crate::chat::ChatService) -> Option<PathBuf> {
    chat.status().setup.cs2_dir.map(PathBuf::from)
}

#[tauri::command]
pub fn gsi_status(
    gsi: tauri::State<'_, Arc<GsiService>>,
    chat: tauri::State<'_, Arc<crate::chat::ChatService>>,
) -> GsiStatus {
    gsi.refresh(cs2_dir(&chat).as_deref())
}

#[tauri::command]
pub fn gsi_install(
    gsi: tauri::State<'_, Arc<GsiService>>,
    chat: tauri::State<'_, Arc<crate::chat::ChatService>>,
) -> Result<GsiStatus, String> {
    let dir = cs2_dir(&chat)
        .ok_or("dossier de CS2 introuvable : indiquez-le dans Langues & glossaire")?;
    gsi.install(&dir)
}

#[tauri::command]
pub fn gsi_uninstall(
    gsi: tauri::State<'_, Arc<GsiService>>,
    chat: tauri::State<'_, Arc<crate::chat::ChatService>>,
) -> Result<GsiStatus, String> {
    let dir = cs2_dir(&chat).ok_or("dossier de CS2 introuvable")?;
    gsi.uninstall(&dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct Rec {
        maps: Mutex<Vec<Option<String>>>,
        statuses: Mutex<usize>,
    }
    impl GsiEvents for Rec {
        fn status(&self, _: &GsiStatus) {
            *self.statuses.lock().unwrap() += 1;
        }
        fn map(&self, map: Option<&str>) {
            self.maps.lock().unwrap().push(map.map(str::to_string));
        }
    }

    fn cs2(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("crosstalk-gsi-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(cfg_dir(&d)).unwrap();
        d
    }

    fn post(port: u16, body: &str) -> String {
        let mut s = TcpStream::connect(("127.0.0.1", port)).unwrap();
        write!(s, "POST / HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}", body.len()).unwrap();
        let mut out = String::new();
        s.read_to_string(&mut out).unwrap();
        out
    }

    #[test]
    fn known_maps_have_their_callouts() {
        let m = &maps()["de_mirage"];
        assert_eq!(m.name, "Mirage");
        assert!(m.prompt.iter().any(|c| c == "Palace"));
        for (k, v) in maps() {
            assert!(v.keep.iter().all(|c| v.prompt.contains(c)), "{k}");
            let len: usize = v.prompt.iter().map(|c| c.len() + 2).sum();
            assert!(len <= 220, "{k}: prompt too long ({len})");
        }
        assert_eq!(pretty("de_dust2"), "Dust II");
        assert_eq!(pretty("cs_office"), "cs_office");
    }

    #[test]
    fn cfg_requests_only_the_map_and_its_token_is_read_back() {
        let t = cfg_text("abc123");
        assert!(t.contains("\"uri\" \"http://127.0.0.1:31982/\""));
        assert!(t.contains("\"map\" \"1\""));
        assert!(!t.contains("player") && !t.contains("allplayers") && !t.contains("round"));
        assert_eq!(token_of(&t).as_deref(), Some("abc123"));
        assert_eq!(token_of("\"Crosstalk\" {}"), None);
        assert_eq!(new_token().len(), 24);
    }

    #[test]
    fn parses_the_map_and_rejects_foreign_messages() {
        let ok = r#"{"provider":{"appid":730},"map":{"name":"de_Mirage","mode":"competitive","phase":"live"},"auth":{"token":"t"}}"#;
        assert_eq!(
            parse_state(ok, "t"),
            Some(Some(("de_mirage".into(), Some("competitive".into()))))
        );
        assert_eq!(parse_state(ok, "other"), None);
        let menu = r#"{"provider":{"appid":730},"auth":{"token":"t"}}"#;
        assert_eq!(parse_state(menu, "t"), Some(None));
        assert_eq!(
            parse_state(r#"{"provider":{"appid":570},"auth":{"token":"t"}}"#, "t"),
            None
        );
        assert_eq!(parse_state("not json", "t"), None);
    }

    #[test]
    fn install_listen_receive_and_uninstall() {
        let dir = cs2("flow");
        let rec = Arc::new(Rec::default());
        let port = 31982 + 7 + (std::process::id() % 500) as u16;
        let gsi = GsiService::with_port(rec.clone(), port);
        let st = gsi.refresh(Some(&dir));
        assert!(st.cs2_found && !st.installed && !st.listening);

        let st = gsi.install(&dir).unwrap();
        assert!(st.installed && st.listening, "{st:?}");
        let text = std::fs::read_to_string(cfg_dir(&dir).join(CFG_NAME)).unwrap();
        let token = token_of(&text).unwrap();

        // A wrong token is refused; the right one sets the map.
        let bad = r#"{"provider":{"appid":730},"map":{"name":"de_nuke"},"auth":{"token":"nope"}}"#;
        assert!(post(port, bad).starts_with("HTTP/1.1 403"));
        let good = format!(
            r#"{{"provider":{{"appid":730}},"map":{{"name":"de_nuke","mode":"competitive"}},"auth":{{"token":"{token}"}}}}"#
        );
        assert!(post(port, &good).starts_with("HTTP/1.1 200"));
        assert!(post(port, &good).starts_with("HTTP/1.1 200"));
        let st = gsi.status();
        assert_eq!(st.map.as_deref(), Some("de_nuke"));
        assert_eq!(st.map_name.as_deref(), Some("Nuke"));
        assert!(st.callouts > 10 && st.last_seen.is_some());
        // Back to the menu, then CS2 closed.
        let menu = format!(r#"{{"provider":{{"appid":730}},"auth":{{"token":"{token}"}}}}"#);
        post(port, &menu);
        assert_eq!(
            *rec.maps.lock().unwrap(),
            vec![Some("de_nuke".to_string()), None]
        );

        // Re-installing keeps the token; removing deletes the file.
        gsi.install(&dir).unwrap();
        assert_eq!(
            token_of(&std::fs::read_to_string(cfg_dir(&dir).join(CFG_NAME)).unwrap()).unwrap(),
            token
        );
        let st = gsi.uninstall(&dir).unwrap();
        assert!(!st.installed && !cfg_dir(&dir).join(CFG_NAME).exists());
        assert!(post(port, &good).starts_with("HTTP/1.1 403"));
    }

    #[test]
    fn install_needs_the_cfg_folder() {
        let gsi = GsiService::with_port(Arc::new(Rec::default()), 1);
        let missing = std::env::temp_dir().join("crosstalk-gsi-nowhere");
        assert!(gsi.install(&missing).unwrap_err().contains("cfg"));
    }
}
