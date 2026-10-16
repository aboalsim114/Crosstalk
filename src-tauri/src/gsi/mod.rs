//! Map being played, from CS2's Game State Integration (A-06, F-36).
//!
//! Game State Integration is Valve's official interface: CS2 reads a
//! `gamestate_integration_*.cfg` file from its `cfg` folder at launch and POSTs
//! the game state as JSON to the address given there. Crosstalk asks only for
//! the map (`"map" "1"`): no player, position or round data. Nothing is read
//! from or written to the game's process. The file is written only when the
//! user clicks "Activer" and removed by "Retirer".
//!
//! The map's callouts are then kept as spoken in translations.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const STATUS_EVENT: &str = "gsi://status";
pub const CFG_NAME: &str = "gamestate_integration_crosstalk.cfg";
/// Local port CS2 posts to (127.0.0.1 only).
pub const PORT: u16 = 31982;
const MAX_BODY: usize = 256 * 1024;
/// A request must arrive whole within this time, however slowly it trickles.
const REQUEST_DEADLINE: Duration = Duration::from_secs(2);
/// Requests read at once: a slow local client never holds up CS2's messages.
const MAX_CONNECTIONS: usize = 4;
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
    /// Match phase from the same `map` block: `warmup`, `live`,
    /// `intermission`, `gameover` (for the end-of-match recap).
    pub phase: Option<String>,
    pub round: Option<u32>,
    pub score_ct: Option<u32>,
    pub score_t: Option<u32>,
}

/// The `map` block of a CS2 message: the map and the match score (no player
/// data is requested).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MapState {
    pub name: String,
    pub mode: Option<String>,
    pub phase: Option<String>,
    pub round: Option<u32>,
    pub score_ct: Option<u32>,
    pub score_t: Option<u32>,
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
pub fn parse_state(body: &str, token: &str) -> Option<Option<MapState>> {
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
    let text = |ptr: &str| {
        v.pointer(ptr)
            .and_then(Value::as_str)
            .map(str::to_string)
            .filter(|s| s.len() <= 32)
    };
    let num = |ptr: &str| {
        v.pointer(ptr)
            .and_then(Value::as_u64)
            .filter(|n| *n <= 1000)
            .map(|n| n as u32)
    };
    Some(name.map(|name| MapState {
        name,
        mode: text("/map/mode"),
        phase: text("/map/phase"),
        round: num("/map/round"),
        score_ct: num("/map/team_ct/score"),
        score_t: num("/map/team_t/score"),
    }))
}

/// Reads one HTTP request (headers + body) from a local connection.
fn read_request(stream: &mut TcpStream) -> Option<String> {
    let end = Instant::now() + REQUEST_DEADLINE;
    let read = |stream: &mut TcpStream, chunk: &mut [u8]| -> Option<usize> {
        let left = end.checked_duration_since(Instant::now())?;
        if left.is_zero() {
            return None;
        }
        stream.set_read_timeout(Some(left)).ok()?;
        stream.read(chunk).ok()
    };
    let mut buf = Vec::with_capacity(4096);
    let mut chunk = [0u8; 4096];
    let header_end = loop {
        let n = read(stream, &mut chunk)?;
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
        let n = read(stream, &mut chunk)?;
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
    /// Requests being read (at most `MAX_CONNECTIONS`).
    connections: AtomicUsize,
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
            connections: AtomicUsize::new(0),
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

    fn set_map(&self, map: Option<MapState>) {
        let (changed, progressed) = {
            let mut st = self.status.lock().unwrap();
            let name = map.as_ref().map(|m| m.name.clone());
            let changed = st.map != name;
            let before = (st.phase.clone(), st.round, st.score_ct, st.score_t);
            let m = map.clone().unwrap_or_default();
            st.phase = m.phase;
            st.round = m.round;
            st.score_ct = m.score_ct;
            st.score_t = m.score_t;
            let progressed = before != (st.phase.clone(), st.round, st.score_ct, st.score_t);
            st.map_name = name.as_deref().map(pretty);
            st.callouts = name
                .as_deref()
                .and_then(|n| maps().get(n))
                .map_or(0, |m| m.prompt.len());
            st.map = name;
            st.mode = map.and_then(|m| m.mode);
            (changed, progressed)
        };
        if changed {
            let map = self.status().map;
            self.events.map(map.as_deref());
        }
        if changed || progressed {
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

    /// Reads one message from CS2 and answers it (403 when refused).
    fn answer(&self, mut stream: TcpStream) {
        let ok = read_request(&mut stream).is_some_and(|b| self.on_message(&b));
        let reply = if ok {
            "HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
        } else {
            "HTTP/1.1 403 Forbidden\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
        };
        let _ = stream.write_all(reply.as_bytes());
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
                            let Ok(stream) = stream else { continue };
                            // Each request on its own thread, a few at once:
                            // beyond that, the connection is closed.
                            if me.connections.fetch_add(1, Ordering::SeqCst) >= MAX_CONNECTIONS {
                                me.connections.fetch_sub(1, Ordering::SeqCst);
                                continue;
                            }
                            let worker = me.clone();
                            let spawned = std::thread::Builder::new()
                                .name("gsi-request".into())
                                .spawn(move || {
                                    worker.answer(stream);
                                    worker.connections.fetch_sub(1, Ordering::SeqCst);
                                });
                            if spawned.is_err() {
                                me.connections.fetch_sub(1, Ordering::SeqCst);
                            }
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
            Some(Some(MapState {
                name: "de_mirage".into(),
                mode: Some("competitive".into()),
                phase: Some("live".into()),
                ..Default::default()
            }))
        );
        // The score of the match, from the same block (no player data).
        let end = r#"{"provider":{"appid":730},"map":{"name":"de_inferno","phase":"gameover","round":22,"team_ct":{"score":13,"name":"x"},"team_t":{"score":9}},"auth":{"token":"t"}}"#;
        let m = parse_state(end, "t").unwrap().unwrap();
        assert_eq!(
            (m.phase.as_deref(), m.round, m.score_ct, m.score_t),
            (Some("gameover"), Some(22), Some(13), Some(9))
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
        // The end of the match reaches the status (for the recap).
        let statuses = *rec.statuses.lock().unwrap();
        let over = format!(
            r#"{{"provider":{{"appid":730}},"map":{{"name":"de_nuke","phase":"gameover","team_ct":{{"score":13}},"team_t":{{"score":11}}}},"auth":{{"token":"{token}"}}}}"#
        );
        post(port, &over);
        let st = gsi.status();
        assert_eq!(
            (st.phase.as_deref(), st.score_ct, st.score_t),
            (Some("gameover"), Some(13), Some(11))
        );
        assert!(*rec.statuses.lock().unwrap() > statuses, "published");
        assert_eq!(rec.maps.lock().unwrap().len(), 1, "same map: no map event");
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
    fn a_slow_local_client_neither_blocks_cs2_nor_stays_connected() {
        let dir = cs2("slow");
        let port = 31982 + 9 + (std::process::id() % 500) as u16;
        let gsi = GsiService::with_port(Arc::new(Rec::default()), port);
        gsi.install(&dir).unwrap();
        let token =
            token_of(&std::fs::read_to_string(cfg_dir(&dir).join(CFG_NAME)).unwrap()).unwrap();
        // A client that sends one byte every 300 ms, forever.
        let mut slow = TcpStream::connect(("127.0.0.1", port)).unwrap();
        let started = Instant::now();
        let trickle = std::thread::spawn(move || {
            for b in b"POST / HTTP/1.1\r\nHost: 127.0.0.1\r\n"
                .iter()
                .cycle()
                .take(40)
            {
                if slow.write_all(&[*b]).is_err() {
                    return Some(started.elapsed());
                }
                std::thread::sleep(Duration::from_millis(300));
            }
            None
        });
        std::thread::sleep(Duration::from_millis(200));
        // CS2's message is answered at once.
        let t = Instant::now();
        let good = format!(
            r#"{{"provider":{{"appid":730}},"map":{{"name":"de_nuke"}},"auth":{{"token":"{token}"}}}}"#
        );
        assert!(post(port, &good).starts_with("HTTP/1.1 200"));
        assert!(t.elapsed() < Duration::from_secs(1), "{:?}", t.elapsed());
        // The slow client is cut off after the deadline (write fails once closed).
        let cut = trickle
            .join()
            .unwrap()
            .expect("slow client never disconnected");
        assert!(cut < Duration::from_secs(5), "{cut:?}");
        let _ = gsi.uninstall(&dir);
    }

    #[test]
    fn install_needs_the_cfg_folder() {
        let gsi = GsiService::with_port(Arc::new(Rec::default()), 1);
        let missing = std::env::temp_dir().join("crosstalk-gsi-nowhere");
        assert!(gsi.install(&missing).unwrap_err().contains("cfg"));
    }
}
