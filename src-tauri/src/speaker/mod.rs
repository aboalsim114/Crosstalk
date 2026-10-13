//! "Qui parle ?" (experimental, off by default): while teammates talk, CS2
//! lists their names on its voice indicator. Crosstalk reads that small area
//! of the screen with Windows' built-in OCR and credits each voice line with
//! the name shown while it was spoken: "Sasha : deux en B".
//!
//! Only pixels already on the user's screen are read (like a screenshot):
//! no access to CS2's process or memory. Frames stay in memory and are
//! dropped after reading; names are kept in memory for the session only and
//! never logged.

pub mod ocr;
pub mod screen;
pub mod track;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};

use screen::{Rect, Region};
use track::Tracker;

pub const STATUS_EVENT: &str = "speaker://status";
/// Readings per second while someone talks.
const READ_EVERY: Duration = Duration::from_millis(250);
/// Keeps reading this long after the voice stops (the indicator lingers).
const AFTER_VOICE: Duration = Duration::from_millis(1500);

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpeakerConfig {
    pub enabled: bool,
    #[serde(default)]
    pub region: Region,
    /// Names never credited (the user's own name: CS2 shows it too).
    #[serde(default)]
    pub ignore: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpeakerStatus {
    pub enabled: bool,
    /// `off`, `waiting` (no CS2 window), `paused` (CS2 in the background),
    /// `watching` or `error`.
    pub state: &'static str,
    pub detail: Option<String>,
    /// Language of Windows' OCR, e.g. `fr-FR`.
    pub ocr_lang: Option<String>,
    /// Names read this session (memory only).
    pub known: Vec<String>,
    /// The last frame was all black (CS2 in exclusive full screen).
    pub blank: bool,
    /// A Crosstalk window covers part of the area.
    pub covered: bool,
}

/// Result of "Tester": what Crosstalk sees and reads, for calibration.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpeakerTest {
    /// `data:image/bmp` of the area (shown in the settings, never saved).
    pub preview: Option<String>,
    pub lines: Vec<String>,
    pub names: Vec<String>,
    pub blank: bool,
    pub covered: bool,
    pub foreground: bool,
    pub error: Option<String>,
}

pub struct SpeakerService {
    cfg: Mutex<SpeakerConfig>,
    tracker: Mutex<Tracker>,
    status: Mutex<SpeakerStatus>,
    last_voice: Mutex<Option<Instant>>,
    speaking: AtomicBool,
}

impl SpeakerService {
    pub fn new() -> Arc<Self> {
        Arc::new(SpeakerService {
            cfg: Mutex::new(SpeakerConfig::default()),
            tracker: Mutex::new(Tracker::default()),
            status: Mutex::new(SpeakerStatus {
                state: "off",
                ..Default::default()
            }),
            last_voice: Mutex::new(None),
            speaking: AtomicBool::new(false),
        })
    }

    pub fn config(&self) -> SpeakerConfig {
        self.cfg.lock().unwrap().clone()
    }

    pub fn configure(&self, cfg: SpeakerConfig) {
        if !cfg.enabled {
            self.tracker.lock().unwrap().clear();
        }
        *self.cfg.lock().unwrap() = cfg;
    }

    pub fn status(&self) -> SpeakerStatus {
        self.status.lock().unwrap().clone()
    }

    /// Voice activity of the game audio (speech VAD).
    pub fn voice(&self, speaking: bool) {
        self.speaking.store(speaking, Ordering::SeqCst);
        if speaking {
            *self.last_voice.lock().unwrap() = Some(Instant::now());
        }
    }

    fn voice_recent(&self) -> bool {
        self.speaking.load(Ordering::SeqCst)
            || self
                .last_voice
                .lock()
                .unwrap()
                .is_some_and(|t| t.elapsed() < AFTER_VOICE)
    }

    /// A chat sender: a name of this match, trusted at its first reading.
    pub fn chat_sender(&self, name: &str) {
        if self.cfg.lock().unwrap().enabled {
            self.tracker.lock().unwrap().confirm(name);
        }
    }

    /// Records one reading; returns the names read.
    pub fn observe(&self, at: Instant, lines: &[String]) -> Vec<String> {
        let ignore = self.cfg.lock().unwrap().ignore.clone();
        let lines: Vec<String> = lines
            .iter()
            .filter(|l| {
                !track::clean_name(l)
                    .is_some_and(|n| ignore.iter().any(|i| i.to_lowercase() == n.to_lowercase()))
            })
            .cloned()
            .collect();
        self.tracker.lock().unwrap().observe(at, &lines)
    }

    /// The name of whoever spoke a voice line that ended `ended_ago` ago and
    /// lasted `length`.
    pub fn speaker_for(&self, ended_ago: Duration, length: Duration) -> Option<String> {
        if !self.cfg.lock().unwrap().enabled {
            return None;
        }
        let now = Instant::now();
        let end = now.checked_sub(ended_ago)?;
        let start = end.checked_sub(length).unwrap_or(end);
        self.tracker.lock().unwrap().speaker_between(start, end)
    }

    /// Updates the status; `true` when it changed.
    fn set_status(&self, f: impl FnOnce(&mut SpeakerStatus)) -> Option<SpeakerStatus> {
        let mut st = self.status.lock().unwrap();
        let before = st.clone();
        f(&mut st);
        st.enabled = self.cfg.lock().unwrap().enabled;
        st.known = self.tracker.lock().unwrap().known();
        (*st != before).then(|| st.clone())
    }
}

/// Crosstalk's always-on-top windows (overlay, reply box): their pixels are
/// not read.
fn own_windows(app: &AppHandle) -> Vec<Rect> {
    [crate::overlay::LABEL, crate::reply::LABEL]
        .iter()
        .filter_map(|l| app.get_webview_window(l))
        .filter(|w| w.is_visible().unwrap_or(false))
        .filter_map(|w| {
            let p = w.outer_position().ok()?;
            let s = w.outer_size().ok()?;
            Some(Rect {
                left: p.x,
                top: p.y,
                right: p.x + s.width as i32,
                bottom: p.y + s.height as i32,
            })
        })
        .collect()
}

struct Reading {
    frame: screen::Frame,
    lines: Vec<String>,
    blank: bool,
    covered: bool,
}

fn read_area(
    window: Rect,
    region: &Region,
    cover: &[Rect],
    ocr: &ocr::Ocr,
) -> Result<Reading, String> {
    let rect = screen::region_rect(window, region).ok_or("zone trop petite")?;
    let mut frame = screen::grab(rect)?;
    let covered = cover.iter().any(|c| c.intersects(&rect));
    screen::mask(&mut frame, rect, cover);
    let blank = screen::is_blank(&frame);
    let lines = if blank {
        Vec::new()
    } else {
        ocr.read(&screen::upscale2(&frame))?
    };
    Ok(Reading {
        frame,
        lines,
        blank,
        covered,
    })
}

/// Starts the reading thread (idle while the feature is off).
pub fn start(app: &AppHandle, svc: Arc<SpeakerService>) {
    let app = app.clone();
    std::thread::Builder::new()
        .name("speaker".into())
        .spawn(move || run(&app, &svc))
        .expect("speaker thread");
}

fn run(app: &AppHandle, svc: &SpeakerService) {
    let emit = |st: Option<SpeakerStatus>| {
        if let Some(st) = st {
            let _ = app.emit(STATUS_EVENT, st);
        }
    };
    let mut engine: Option<ocr::Ocr> = None;
    let mut was_enabled = false;
    loop {
        let cfg = svc.config();
        if !cfg.enabled {
            engine = None;
            was_enabled = false;
            emit(svc.set_status(|s| {
                *s = SpeakerStatus {
                    state: "off",
                    ..Default::default()
                }
            }));
            std::thread::sleep(Duration::from_millis(500));
            continue;
        }
        if engine.is_none() {
            match ocr::Ocr::new() {
                Ok(o) => {
                    if !was_enabled {
                        crate::logs::info(
                            "speaker",
                            &format!("identification des voix activée (OCR Windows {})", o.lang),
                        );
                    }
                    let lang = o.lang.clone();
                    emit(svc.set_status(|s| s.ocr_lang = Some(lang)));
                    engine = Some(o);
                }
                Err(e) => {
                    if !was_enabled {
                        crate::logs::warn("speaker", &e);
                    }
                    was_enabled = true;
                    emit(svc.set_status(|s| {
                        s.state = "error";
                        s.detail = Some(e);
                    }));
                    std::thread::sleep(Duration::from_secs(5));
                    continue;
                }
            }
        }
        was_enabled = true;
        let ocr = engine.as_ref().expect("engine");
        let window = match screen::cs2_window() {
            None => {
                emit(svc.set_status(|s| {
                    s.state = "waiting";
                    s.detail = Some("CS2 n'est pas ouvert (ou il est réduit)".into());
                }));
                std::thread::sleep(Duration::from_secs(1));
                continue;
            }
            Some((_, false)) => {
                emit(svc.set_status(|s| {
                    s.state = "paused";
                    s.detail = Some("CS2 est en arrière-plan".into());
                }));
                std::thread::sleep(Duration::from_millis(500));
                continue;
            }
            Some((w, true)) => w,
        };
        if !svc.voice_recent() {
            emit(svc.set_status(|s| {
                s.state = "watching";
                s.detail = None;
            }));
            std::thread::sleep(Duration::from_millis(100));
            continue;
        }
        let started = Instant::now();
        match read_area(window, &cfg.region, &own_windows(app), ocr) {
            Ok(r) => {
                svc.observe(started, &r.lines);
                emit(svc.set_status(|s| {
                    s.state = "watching";
                    s.detail = None;
                    s.blank = r.blank;
                    s.covered = r.covered;
                }));
            }
            Err(e) => emit(svc.set_status(|s| {
                s.state = "error";
                s.detail = Some(e);
            })),
        }
        std::thread::sleep(READ_EVERY.saturating_sub(started.elapsed()));
    }
}

#[tauri::command]
pub fn speaker_status(svc: tauri::State<'_, Arc<SpeakerService>>) -> SpeakerStatus {
    svc.status()
}

#[tauri::command]
pub fn speaker_configure(
    svc: tauri::State<'_, Arc<SpeakerService>>,
    config: SpeakerConfig,
) -> SpeakerStatus {
    svc.configure(config);
    svc.status()
}

/// Reads the area once after `delay_ms` (time to switch to CS2 and hold the
/// voice key), and shows what was seen.
#[tauri::command]
pub async fn speaker_test(app: AppHandle, region: Region, delay_ms: Option<u32>) -> SpeakerTest {
    let delay = Duration::from_millis(delay_ms.unwrap_or(3000).min(10_000) as u64);
    tauri::async_runtime::spawn_blocking(move || {
        std::thread::sleep(delay);
        let fail = |e: String| SpeakerTest {
            error: Some(e),
            ..Default::default()
        };
        let ocr = match ocr::Ocr::new() {
            Ok(o) => o,
            Err(e) => return fail(e),
        };
        let Some((window, foreground)) = screen::cs2_window() else {
            return fail("CS2 n'est pas ouvert (ou il est réduit)".into());
        };
        match read_area(window, &region, &own_windows(&app), &ocr) {
            Ok(r) => SpeakerTest {
                preview: Some(screen::bmp_data_url(&r.frame)),
                names: r
                    .lines
                    .iter()
                    .filter_map(|l| track::clean_name(l))
                    .collect(),
                lines: r.lines,
                blank: r.blank,
                covered: r.covered,
                foreground,
                error: None,
            },
            Err(e) => fail(e),
        }
    })
    .await
    .unwrap_or_else(|e| SpeakerTest {
        error: Some(e.to_string()),
        ..Default::default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn credits_lines_only_while_enabled_and_skips_the_users_own_name() {
        let svc = SpeakerService::new();
        let now = Instant::now();
        for i in 0..4u64 {
            svc.observe(
                now - Duration::from_millis(1200 - i * 300),
                &lines(&["Player One"]),
            );
        }
        assert_eq!(
            svc.speaker_for(Duration::ZERO, Duration::from_secs(1)),
            None,
            "off"
        );
        svc.configure(SpeakerConfig {
            enabled: true,
            ignore: vec!["moi".into()],
            ..Default::default()
        });
        for i in 0..4u64 {
            svc.observe(
                now - Duration::from_millis(1200 - i * 300),
                &lines(&["Player One"]),
            );
        }
        assert_eq!(
            svc.speaker_for(Duration::ZERO, Duration::from_secs(1))
                .as_deref(),
            Some("Player One")
        );
        // The user's own name, shown while both talk, is not a rival.
        for i in 0..4u64 {
            svc.observe(
                now - Duration::from_millis(1100 - i * 300),
                &lines(&["Moi"]),
            );
        }
        assert_eq!(
            svc.speaker_for(Duration::ZERO, Duration::from_secs(1))
                .as_deref(),
            Some("Player One")
        );
        assert!(
            svc.set_status(|_| {}).is_some(),
            "known names reach the status"
        );
        svc.configure(SpeakerConfig::default());
        assert!(
            svc.tracker.lock().unwrap().known().is_empty(),
            "forgotten when turned off"
        );
    }

    #[test]
    fn reads_only_around_voice() {
        let svc = SpeakerService::new();
        assert!(!svc.voice_recent());
        svc.voice(true);
        assert!(svc.voice_recent());
        svc.voice(false);
        assert!(svc.voice_recent(), "the indicator lingers after the voice");
        *svc.last_voice.lock().unwrap() = Some(Instant::now() - Duration::from_secs(2));
        assert!(!svc.voice_recent());
    }

    #[test]
    fn config_from_the_ui() {
        let c: SpeakerConfig = serde_json::from_str(r#"{"enabled":true}"#).unwrap();
        assert_eq!(c.region, Region::default());
        assert!(c.ignore.is_empty());
    }
}
