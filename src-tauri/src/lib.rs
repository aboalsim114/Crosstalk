//! Crosstalk desktop core.
//!
//! The React UI (../src) is embedded in this binary and rendered by WebView2.
//! All native features live here, behind Tauri commands, events and plugins:
//!
//! - `tray`      notification-area icon; the app keeps running when the window is closed
//! - `shortcuts` system-wide Ctrl+Alt+F8 / F9 / F10
//! - `overlay`   transparent, always-on-top, click-through subtitle window
//! - autostart   optional launch with Windows (`--minimized`, straight to the tray)
//! - `pipeline`  seams for audio capture, speech recognition, translation and CS2

mod actions;
mod commands;
mod overlay;
// Traits and helpers for the upcoming native pipeline; wired in later phases.
#[allow(dead_code)]
mod pipeline;
mod shortcuts;
mod tray;

use tauri::WindowEvent;
use tauri_plugin_autostart::MacosLauncher;

/// Passed by the autostart entry so Windows login starts Crosstalk in the tray.
pub const MINIMIZED_FLAG: &str = "--minimized";

pub fn run() {
    tauri::Builder::default()
        // Must be first: a second launch just focuses the running instance.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            actions::show_main(app)
        }))
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            Some(vec![MINIMIZED_FLAG]),
        ))
        .plugin(shortcuts::plugin())
        .manage(shortcuts::ShortcutRegistry::default())
        .invoke_handler(tauri::generate_handler![
            commands::app_info,
            overlay::overlay_configure,
            shortcuts::shortcut_status
        ])
        .setup(|app| {
            let handle = app.handle();
            tray::create(handle)?;
            shortcuts::register_all(handle);
            overlay::create(handle)?;
            if !std::env::args().any(|a| a == MINIMIZED_FLAG) {
                actions::show_main(handle);
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            // Closing the main window hides it to the tray; "Quitter" in the tray exits.
            if let WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == actions::MAIN_WINDOW {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running Crosstalk");
}
