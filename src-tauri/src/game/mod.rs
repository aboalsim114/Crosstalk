//! Counter-Strike 2 detection.

pub mod process;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::Serialize;

/// Event sent to the UI when CS2 starts or exits.
pub const STATUS_EVENT: &str = "cs2://status";

/// Polling period of the watcher (cahier des charges F-02).
pub const POLL_INTERVAL: Duration = Duration::from_secs(2);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Cs2Status {
    pub running: bool,
    pub pid: Option<u32>,
}

impl Cs2Status {
    pub fn from_pid(pid: Option<u32>) -> Self {
        Self {
            running: pid.is_some(),
            pid,
        }
    }
}

/// What changed between two polls.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cs2Change {
    Started(u32),
    Exited(u32),
    /// The game restarted between two polls (new PID).
    Restarted {
        old: u32,
        new: u32,
    },
}

pub fn diff(before: Option<u32>, after: Option<u32>) -> Option<Cs2Change> {
    match (before, after) {
        (None, Some(p)) => Some(Cs2Change::Started(p)),
        (Some(p), None) => Some(Cs2Change::Exited(p)),
        (Some(a), Some(b)) if a != b => Some(Cs2Change::Restarted { old: a, new: b }),
        _ => None,
    }
}

/// Latest known CS2 state, shared with the commands.
#[derive(Default)]
pub struct Cs2Watcher {
    current: Mutex<Option<u32>>,
}

impl Cs2Watcher {
    pub fn status(&self) -> Cs2Status {
        Cs2Status::from_pid(*self.current.lock().unwrap())
    }

    /// Polls once; returns the change, if any.
    pub fn poll(&self) -> Option<Cs2Change> {
        let pid = process::list_processes()
            .ok()
            .and_then(|list| process::find_pid(&list, process::CS2_EXE));
        let mut current = self.current.lock().unwrap();
        let change = diff(*current, pid);
        *current = pid;
        change
    }

    /// Polls forever on a background thread and calls `on_change` on changes.
    pub fn spawn(self: &Arc<Self>, on_change: impl Fn(Cs2Change, Cs2Status) + Send + 'static) {
        let me = self.clone();
        std::thread::Builder::new()
            .name("cs2-watcher".into())
            .spawn(move || loop {
                if let Some(change) = me.poll() {
                    on_change(change, me.status());
                }
                std::thread::sleep(POLL_INTERVAL);
            })
            .expect("spawn cs2 watcher thread");
    }
}

#[tauri::command]
pub fn cs2_status(watcher: tauri::State<'_, Arc<Cs2Watcher>>) -> Cs2Status {
    watcher.status()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_start_exit_and_restart() {
        assert_eq!(diff(None, Some(5)), Some(Cs2Change::Started(5)));
        assert_eq!(diff(Some(5), None), Some(Cs2Change::Exited(5)));
        assert_eq!(
            diff(Some(5), Some(6)),
            Some(Cs2Change::Restarted { old: 5, new: 6 })
        );
        assert_eq!(diff(Some(5), Some(5)), None);
        assert_eq!(diff(None, None), None);
    }

    #[test]
    fn status_serialises_for_the_ui() {
        let json = serde_json::to_value(Cs2Status::from_pid(Some(42))).unwrap();
        assert_eq!(json, serde_json::json!({ "running": true, "pid": 42 }));
        assert!(!Cs2Status::from_pid(None).running);
    }
}
