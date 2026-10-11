//! Crosstalk desktop core.
//!
//! The React UI (../src) is embedded in this binary and rendered by WebView2.
//! All native features live here, behind Tauri commands.

mod commands;

pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![commands::app_info])
        .run(tauri::generate_context!())
        .expect("error while running Crosstalk");
}
