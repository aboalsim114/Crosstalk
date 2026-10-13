//! Mode streamer: translated subtitles for OBS (or any streaming software).
//!
//! When the user turns it on, Crosstalk serves a transparent page on
//! `http://127.0.0.1:31983/` that OBS shows as a "Browser source"; final
//! subtitles are pushed to it live (Server-Sent Events). Local only: bound to
//! 127.0.0.1, other Host headers refused (DNS rebinding), GET only. Player
//! names and original sentences are removed here, before they reach the page,
//! unless the user chose to show them. Off by default; nothing is stored.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{channel, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::{Deserialize, Serialize};

pub const PORT: u16 = 31983;
const MAX_CLIENTS: usize = 8;
const PAGE: &str = include_str!("page.html");

/// A final subtitle, as the main window sends it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamLine {
    /// `voice` or `chat`.
    pub kind: String,
    pub lang: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channel: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sender: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub original: Option<String>,
    /// French text; `None` when the line was already in French or untranslated.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub translated: Option<String>,
    /// Sent by the "Ligne de test" button.
    #[serde(default)]
    pub test: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamOptions {
    pub enabled: bool,
    /// Show who spoke or wrote (off: names are personal data).
    pub names: bool,
    /// Show the original sentence under the French one.
    pub original: bool,
    /// Small "Crosstalk" mention in the corner.
    pub watermark: bool,
    /// Text size in px (OBS canvas pixels).
    pub size: u32,
    /// Lines shown at once.
    pub max: u32,
    /// Seconds a line stays on screen.
    pub duration: u32,
}

impl Default for StreamOptions {
    fn default() -> Self {
        Self {
            enabled: false,
            names: false,
            original: false,
            watermark: true,
            size: 34,
            max: 3,
            duration: 8,
        }
    }
}

impl StreamOptions {
    fn sanitized(mut self) -> Self {
        self.size = self.size.clamp(18, 72);
        self.max = self.max.clamp(1, 5);
        self.duration = self.duration.clamp(3, 20);
        self
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamStatus {
    pub options: StreamOptions,
    pub listening: bool,
    pub url: String,
    pub clients: usize,
    pub sent: u64,
    pub error: Option<String>,
}

/// What reaches the page: names and originals removed unless shown.
pub fn for_page(line: &StreamLine, opts: &StreamOptions) -> Option<StreamLine> {
    let text = line.translated.as_deref().or(line.original.as_deref())?;
    if text.trim().is_empty() {
        return None;
    }
    let mut out = line.clone();
    if !opts.names {
        out.sender = None;
    }
    if line.translated.is_none() {
        // Already in French (or untranslated): the original is the subtitle.
        out.translated = line.original.clone();
        out.original = None;
    } else if !opts.original {
        out.original = None;
    }
    let cut = |s: &mut Option<String>| {
        if let Some(t) = s {
            if t.chars().count() > 300 {
                *t = t.chars().take(300).collect::<String>() + "…";
            }
        }
    };
    cut(&mut out.translated);
    cut(&mut out.original);
    Some(out)
}

fn sse(event: &str, data: &str) -> String {
    format!("event: {event}\ndata: {data}\n\n")
}

pub struct StreamService {
    options: Mutex<StreamOptions>,
    clients: Mutex<Vec<Sender<String>>>,
    started: AtomicBool,
    error: Mutex<Option<String>>,
    sent: AtomicU64,
    port: u16,
}

impl StreamService {
    pub fn new() -> Arc<Self> {
        Self::with_port(PORT)
    }

    pub fn with_port(port: u16) -> Arc<Self> {
        Arc::new(Self {
            options: Mutex::new(StreamOptions::default()),
            clients: Mutex::new(Vec::new()),
            started: AtomicBool::new(false),
            error: Mutex::new(None),
            sent: AtomicU64::new(0),
            port,
        })
    }

    pub fn url(&self) -> String {
        format!("http://127.0.0.1:{}/", self.port)
    }

    pub fn status(&self) -> StreamStatus {
        StreamStatus {
            options: self.options.lock().unwrap().clone(),
            listening: self.started.load(Ordering::SeqCst),
            url: self.url(),
            clients: self.clients.lock().unwrap().len(),
            sent: self.sent.load(Ordering::SeqCst),
            error: self.error.lock().unwrap().clone(),
        }
    }

    fn config_event(&self) -> String {
        let o = self.options.lock().unwrap().clone();
        sse("config", &serde_json::to_string(&o).unwrap_or_default())
    }

    fn broadcast(&self, msg: &str) {
        self.clients
            .lock()
            .unwrap()
            .retain(|c| c.send(msg.to_string()).is_ok());
    }

    pub fn configure(self: &Arc<Self>, options: StreamOptions) -> StreamStatus {
        let options = options.sanitized();
        let enabled = options.enabled;
        *self.options.lock().unwrap() = options;
        if enabled {
            self.listen();
        }
        self.broadcast(&self.config_event());
        self.status()
    }

    /// A final subtitle; ignored while the mode is off.
    pub fn push(&self, line: &StreamLine) -> bool {
        let opts = self.options.lock().unwrap().clone();
        if !opts.enabled {
            return false;
        }
        let Some(out) = for_page(line, &opts) else {
            return false;
        };
        self.sent.fetch_add(1, Ordering::SeqCst);
        self.broadcast(&sse(
            "line",
            &serde_json::to_string(&out).unwrap_or_default(),
        ));
        true
    }

    fn listen(self: &Arc<Self>) {
        if self.started.swap(true, Ordering::SeqCst) {
            return;
        }
        match TcpListener::bind(("127.0.0.1", self.port)) {
            Ok(listener) => {
                *self.error.lock().unwrap() = None;
                let me = self.clone();
                std::thread::Builder::new()
                    .name("stream".into())
                    .spawn(move || {
                        for stream in listener.incoming().flatten() {
                            let me = me.clone();
                            let _ = std::thread::Builder::new()
                                .name("stream-client".into())
                                .spawn(move || me.serve(stream));
                        }
                    })
                    .expect("stream thread");
            }
            Err(e) => {
                self.started.store(false, Ordering::SeqCst);
                *self.error.lock().unwrap() = Some(format!(
                    "port local {} indisponible ({e}) : une autre application l’utilise",
                    self.port
                ));
            }
        }
    }

    fn serve(&self, mut stream: TcpStream) {
        let _ = stream.set_read_timeout(Some(Duration::from_secs(3)));
        let Some((path, host)) = read_get(&mut stream) else {
            let _ = stream.write_all(
                b"HTTP/1.1 400 Bad Request\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
            );
            return;
        };
        let local = [
            format!("127.0.0.1:{}", self.port),
            format!("localhost:{}", self.port),
        ];
        if !local.contains(&host) {
            let _ = stream.write_all(
                b"HTTP/1.1 403 Forbidden\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
            );
            return;
        }
        match path.split('?').next().unwrap_or("") {
            "/" | "/overlay" => {
                let head = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
                    PAGE.len()
                );
                let _ = stream.write_all(head.as_bytes());
                let _ = stream.write_all(PAGE.as_bytes());
            }
            "/events" => self.events(stream),
            _ => {
                let _ = stream.write_all(
                    b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                );
            }
        }
    }

    fn events(&self, mut stream: TcpStream) {
        let (tx, rx) = channel::<String>();
        {
            let mut clients = self.clients.lock().unwrap();
            if clients.len() >= MAX_CLIENTS {
                let _ = stream.write_all(b"HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
                return;
            }
            clients.push(tx);
        }
        let head = "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream; charset=utf-8\r\nCache-Control: no-cache\r\nConnection: keep-alive\r\n\r\nretry: 2000\n\n";
        if stream.write_all(head.as_bytes()).is_err()
            || stream.write_all(self.config_event().as_bytes()).is_err()
        {
            return;
        }
        loop {
            let msg = match rx.recv_timeout(Duration::from_secs(15)) {
                Ok(m) => m,
                Err(RecvTimeoutError::Timeout) => ": ping\n\n".into(),
                Err(RecvTimeoutError::Disconnected) => return,
            };
            if stream
                .write_all(msg.as_bytes())
                .and_then(|_| stream.flush())
                .is_err()
            {
                // OBS closed the source: its sender is dropped at the next broadcast.
                drop(rx);
                self.broadcast(": bye\n\n");
                return;
            }
        }
    }
}

/// Path and Host of a GET request.
fn read_get(stream: &mut TcpStream) -> Option<(String, String)> {
    let mut buf = Vec::with_capacity(1024);
    let mut chunk = [0u8; 1024];
    while !buf.windows(4).any(|w| w == b"\r\n\r\n") {
        let n = stream.read(&mut chunk).ok()?;
        if n == 0 || buf.len() > 8 * 1024 {
            return None;
        }
        buf.extend_from_slice(&chunk[..n]);
    }
    let head = String::from_utf8_lossy(&buf);
    let mut lines = head.lines();
    let mut first = lines.next()?.split_whitespace();
    if first.next()? != "GET" {
        return None;
    }
    let path = first.next()?.to_string();
    let host = lines
        .find_map(|l| {
            let (k, v) = l.split_once(':')?;
            k.trim()
                .eq_ignore_ascii_case("host")
                .then(|| v.trim().to_ascii_lowercase())
        })
        .unwrap_or_default();
    Some((path, host))
}

#[tauri::command]
pub fn stream_status(stream: tauri::State<'_, Arc<StreamService>>) -> StreamStatus {
    stream.status()
}

#[tauri::command]
pub fn stream_configure(
    stream: tauri::State<'_, Arc<StreamService>>,
    options: StreamOptions,
) -> StreamStatus {
    stream.configure(options)
}

#[tauri::command]
pub fn stream_push(stream: tauri::State<'_, Arc<StreamService>>, line: StreamLine) -> bool {
    stream.push(&line)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::BufRead;

    fn line() -> StreamLine {
        StreamLine {
            kind: "voice".into(),
            lang: "ru".into(),
            channel: None,
            sender: Some("Sasha".into()),
            original: Some("Двое на Б".into()),
            translated: Some("Deux sur B".into()),
            test: false,
        }
    }

    fn get(port: u16, path: &str, host: &str) -> String {
        let mut s = TcpStream::connect(("127.0.0.1", port)).unwrap();
        write!(s, "GET {path} HTTP/1.1\r\nHost: {host}\r\n\r\n").unwrap();
        let mut out = String::new();
        s.read_to_string(&mut out).unwrap();
        out
    }

    #[test]
    fn names_and_originals_stay_private_unless_shown() {
        let opts = StreamOptions {
            enabled: true,
            ..Default::default()
        };
        let out = for_page(&line(), &opts).unwrap();
        assert_eq!(out.sender, None);
        assert_eq!(out.original, None);
        assert_eq!(out.translated.as_deref(), Some("Deux sur B"));
        let all = StreamOptions {
            enabled: true,
            names: true,
            original: true,
            ..Default::default()
        };
        let out = for_page(&line(), &all).unwrap();
        assert_eq!(out.sender.as_deref(), Some("Sasha"));
        assert_eq!(out.original.as_deref(), Some("Двое на Б"));
        // Already French: the original is the subtitle.
        let fr = StreamLine {
            translated: None,
            original: Some("on rush B".into()),
            ..line()
        };
        let out = for_page(&fr, &all).unwrap();
        assert_eq!(
            (out.translated.as_deref(), out.original),
            (Some("on rush B"), None)
        );
        assert!(for_page(
            &StreamLine {
                translated: None,
                original: None,
                ..line()
            },
            &all
        )
        .is_none());
        let long = StreamLine {
            translated: Some("a".repeat(400)),
            ..line()
        };
        assert_eq!(
            for_page(&long, &opts)
                .unwrap()
                .translated
                .unwrap()
                .chars()
                .count(),
            301
        );
    }

    #[test]
    fn options_are_bounded() {
        let o = StreamOptions {
            size: 500,
            max: 0,
            duration: 99,
            ..Default::default()
        }
        .sanitized();
        assert_eq!((o.size, o.max, o.duration), (72, 1, 20));
        let d = StreamOptions::default();
        assert!(!d.enabled && !d.names && !d.original && d.watermark);
    }

    #[test]
    fn serves_the_page_and_pushes_lines_live() {
        let port = 31983 + 11 + (std::process::id() % 400) as u16;
        let svc = StreamService::with_port(port);
        assert!(!svc.push(&line()), "off by default");
        let st = svc.configure(StreamOptions {
            enabled: true,
            ..Default::default()
        });
        assert!(st.listening, "{st:?}");
        assert_eq!(st.url, format!("http://127.0.0.1:{port}/"));

        let host = format!("127.0.0.1:{port}");
        let page = get(port, "/", &host);
        assert!(page.starts_with("HTTP/1.1 200") && page.contains("EventSource"));
        assert!(get(port, "/", &format!("evil.example:{port}")).starts_with("HTTP/1.1 403"));
        assert!(get(port, "/nope", &host).starts_with("HTTP/1.1 404"));

        let mut s = TcpStream::connect(("127.0.0.1", port)).unwrap();
        write!(s, "GET /events HTTP/1.1\r\nHost: localhost:{port}\r\n\r\n").unwrap();
        let mut r = std::io::BufReader::new(s.try_clone().unwrap());
        let mut read_until = |needle: &str| {
            let mut seen = String::new();
            for _ in 0..50 {
                let mut l = String::new();
                r.read_line(&mut l).unwrap();
                seen.push_str(&l);
                if l.contains(needle) {
                    return seen;
                }
            }
            panic!("{needle} not found in {seen}");
        };
        assert!(read_until("event: config").contains("text/event-stream"));
        let end = std::time::Instant::now() + Duration::from_secs(3);
        while svc.status().clients == 0 {
            assert!(std::time::Instant::now() < end);
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(svc.push(&line()));
        let got = read_until("Deux sur B");
        assert!(
            got.contains("event: line") && !got.contains("Sasha"),
            "{got}"
        );
        // Turning it off: the page is told, lines are no longer sent.
        svc.configure(StreamOptions::default());
        assert!(read_until("\"enabled\":false").contains("event: config"));
        assert!(!svc.push(&line()));
        assert_eq!(svc.status().sent, 1);
    }
}
