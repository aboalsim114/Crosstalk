//! User actions that can come from outside the UI (global shortcuts, tray menu)
//! and are forwarded to the main window, which owns the session state.

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

/// Event name the UI listens to (see `src/backend/index.ts`).
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
}

pub fn dispatch(app: &AppHandle, action: Action) {
    if let Err(e) = app.emit_to(MAIN_WINDOW, ACTION_EVENT, action) {
        eprintln!("crosstalk: failed to forward {action:?}: {e}");
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
