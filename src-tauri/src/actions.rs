//! User actions that can come from outside the UI (global shortcuts, tray menu)
//! and are forwarded to the main window, which owns the session state.

use std::sync::Arc;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::audio::{AudioService, StopReason};

/// Event name the UI listens to (see `src/shared/ipc/actions.ts`).
pub const ACTION_EVENT: &str = "crosstalk://action";
pub const MAIN_WINDOW: &str = "main";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Action {
    /// Ctrl+Alt+F8: show / hide the in-game overlay.
    Overlay,
    /// Ctrl+Alt+F9: pause / resume the session.
    Pause,
    /// Ctrl+Alt+F10: stop every capture and purge audio buffers.
    Emergency,
    /// Start or stop the session (tray menu).
    Session,
    /// Mouse 4 or Ctrl+Alt+F7: reply in the teammates' language (pop-up over the game).
    Reply,
    /// Mouse 5 or Ctrl+Alt+F6: dictate the reply (microphone), tap again to finish.
    Dictate,
}

pub fn dispatch(app: &AppHandle, action: Action) {
    // The emergency stop cuts the capture here, in Rust, without waiting for the
    // UI: it must take effect in under 200 ms even if the window is hidden.
    if action == Action::Emergency {
        if let Some(audio) = app.try_state::<Arc<AudioService>>() {
            audio.stop(StopReason::Emergency);
        }
    }
    if let Err(e) = app.emit_to(MAIN_WINDOW, ACTION_EVENT, action) {
        crate::logs::warn("actions", &format!("{action:?} non transmis : {e}"));
    }
}

/// Brings the main window back from the tray / taskbar and focuses it.
pub fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window(MAIN_WINDOW) {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn actions_serialise_as_lowercase_names() {
        assert_eq!(
            serde_json::to_string(&Action::Overlay).unwrap(),
            "\"overlay\""
        );
        assert_eq!(
            serde_json::to_string(&Action::Emergency).unwrap(),
            "\"emergency\""
        );
        assert_eq!(
            serde_json::to_string(&Action::Session).unwrap(),
            "\"session\""
        );
    }
}
