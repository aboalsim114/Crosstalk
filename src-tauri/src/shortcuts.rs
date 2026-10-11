//! System-wide hotkeys, active even while CS2 has the focus.
//!
//! Registered with `RegisterHotKey` through tauri-plugin-global-shortcut: no keyboard
//! hook and nothing touches the game process.

use std::sync::Mutex;

use serde::Serialize;
use tauri::{AppHandle, Manager, Wry};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

use crate::actions::{self, Action};

pub const BINDINGS: [(Code, Action, &str); 3] = [
    (Code::F8, Action::Overlay, "Ctrl+Alt+F8"),
    (Code::F9, Action::Pause, "Ctrl+Alt+F9"),
    (Code::F10, Action::Emergency, "Ctrl+Alt+F10"),
];

fn shortcut(code: Code) -> Shortcut {
    Shortcut::new(Some(Modifiers::CONTROL | Modifiers::ALT), code)
}

/// Maps a pressed shortcut back to its action.
pub fn action_for(sc: &Shortcut) -> Option<Action> {
    BINDINGS
        .iter()
        .find(|(code, _, _)| shortcut(*code) == *sc)
        .map(|(_, a, _)| *a)
}

#[derive(Debug, Clone, Serialize)]
pub struct ShortcutStatus {
    pub keys: &'static str,
    pub registered: bool,
    pub error: Option<String>,
}

/// Registration results, shown on the Diagnostic screen.
#[derive(Default)]
pub struct ShortcutRegistry(pub Mutex<Vec<ShortcutStatus>>);

pub fn plugin() -> tauri::plugin::TauriPlugin<Wry> {
    tauri_plugin_global_shortcut::Builder::new()
        .with_handler(|app, sc, event| {
            if event.state() != ShortcutState::Pressed {
                return;
            }
            if let Some(action) = action_for(sc) {
                actions::dispatch(app, action);
            }
        })
        .build()
}

/// Registers every binding; a conflict with another app is reported, not fatal.
pub fn register_all(app: &AppHandle) {
    let gs = app.global_shortcut();
    let statuses = BINDINGS
        .iter()
        .map(|(code, _, keys)| match gs.register(shortcut(*code)) {
            Ok(()) => ShortcutStatus {
                keys,
                registered: true,
                error: None,
            },
            Err(e) => ShortcutStatus {
                keys,
                registered: false,
                error: Some(e.to_string()),
            },
        })
        .collect();
    *app.state::<ShortcutRegistry>().0.lock().unwrap() = statuses;
}

#[tauri::command]
pub fn shortcut_status(registry: tauri::State<'_, ShortcutRegistry>) -> Vec<ShortcutStatus> {
    registry.0.lock().unwrap().clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_ctrl_alt_function_keys_to_actions() {
        assert_eq!(action_for(&shortcut(Code::F8)), Some(Action::Overlay));
        assert_eq!(action_for(&shortcut(Code::F9)), Some(Action::Pause));
        assert_eq!(action_for(&shortcut(Code::F10)), Some(Action::Emergency));
    }

    #[test]
    fn ignores_other_combinations() {
        assert_eq!(
            action_for(&Shortcut::new(Some(Modifiers::CONTROL), Code::F8)),
            None
        );
        assert_eq!(action_for(&shortcut(Code::F11)), None);
    }
}
