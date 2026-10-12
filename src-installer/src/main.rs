//! Crosstalk installer: the "Crosstalk Installer" design (installer.html)
//! driving a real per-user installation (src/install.rs). The NSIS wrapper
//! (scripts/cross/setup-wrapper.nsi) starts it and writes the uninstaller.
//!
//! `--silent [--dir <folder>] [--no-desktop] [--skip-models]` installs without
//! a window (NSIS `/S`); the exit code is 0 when installed, 1 otherwise.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tauri::{Emitter, Manager};

// The app's model catalogues and download code, compiled in unchanged.
#[allow(dead_code)]
#[path = "../../src-tauri/src/downloads.rs"]
mod downloads;
#[allow(dead_code)]
#[path = "../../src-tauri/src/translation/models.rs"]
mod opus_models;
#[allow(dead_code)]
#[path = "../../src-tauri/src/speech/models.rs"]
mod whisper_models;
mod speech {
    pub(crate) use crate::whisper_models as models;
}
mod translation {
    pub(crate) use crate::opus_models as models;
}

mod install;
mod system;

mod payload {
    use crate::install::Entry;
    include!(concat!(env!("OUT_DIR"), "/payload.rs"));
}

use install::{Installer, Options, Stop};

const VERSION: &str = env!("CROSSTALK_VERSION");
const PROGRESS: &str = "setup://progress";

fn installer() -> Arc<Installer> {
    Installer::new(
        payload::FILES,
        speech::models::models_dir(),
        VERSION,
        Arc::new(downloads::HttpFetcher),
        Arc::new(system::System),
    )
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SetupInfo {
    version: &'static str,
    default_dir: PathBuf,
    app_bytes: u64,
    model_bytes: u64,
    installed: Option<String>,
}

#[derive(Deserialize)]
struct InstallOptions {
    dir: PathBuf,
    desktop: bool,
}

#[tauri::command]
fn setup_info(inst: tauri::State<'_, Arc<Installer>>) -> SetupInfo {
    SetupInfo {
        version: VERSION,
        default_dir: system::default_dir(),
        app_bytes: inst.app_bytes(),
        model_bytes: inst.model_bytes(),
        installed: system::installed_version(),
    }
}

#[tauri::command]
fn setup_free_space(inst: tauri::State<'_, Arc<Installer>>, dir: PathBuf) -> Option<u64> {
    inst.platform().free_space(&dir)
}

/// Native folder picker; like every installer, the app gets its own
/// "Crosstalk" sub-folder in the chosen one.
#[tauri::command]
async fn setup_pick_folder(app: tauri::AppHandle, current: PathBuf) -> Option<PathBuf> {
    use tauri_plugin_dialog::DialogExt;
    let start = current.ancestors().find(|p| p.is_dir()).map(PathBuf::from);
    let mut dialog = app
        .dialog()
        .file()
        .set_title("Dossier d’installation de Crosstalk");
    if let Some(s) = start {
        dialog = dialog.set_directory(s);
    }
    let picked = dialog.blocking_pick_folder()?.into_path().ok()?;
    let named = picked
        .file_name()
        .is_some_and(|n| n.to_string_lossy().eq_ignore_ascii_case("crosstalk"));
    Some(if named {
        picked
    } else {
        picked.join("Crosstalk")
    })
}

#[tauri::command]
fn setup_install(
    app: tauri::AppHandle,
    inst: tauri::State<'_, Arc<Installer>>,
    opts: InstallOptions,
) {
    let inst = inst.inner().clone();
    std::thread::Builder::new()
        .name("install".into())
        .spawn(move || {
            let opts = Options {
                dir: opts.dir,
                desktop: opts.desktop,
                models: true,
            };
            let _ = inst.run(&opts, &|p| {
                let _ = app.emit(PROGRESS, p);
            });
        })
        .expect("install thread");
}

#[tauri::command]
fn setup_cancel(inst: tauri::State<'_, Arc<Installer>>) {
    inst.cancel();
}

#[tauri::command]
fn setup_launch(dir: PathBuf) -> Result<(), String> {
    std::process::Command::new(dir.join(install::APP_EXE))
        .current_dir(&dir)
        .spawn()
        .map(drop)
        .map_err(|e| format!("Lancement impossible : {e}"))
}

#[tauri::command]
fn setup_autostart(dir: PathBuf, on: bool) -> Result<(), String> {
    system::set_autostart(&dir.join(install::APP_EXE), on)
}

/// Exit code read by the NSIS wrapper: 0 writes the uninstaller.
#[tauri::command]
fn setup_quit(app: tauri::AppHandle, inst: tauri::State<'_, Arc<Installer>>) {
    inst.cancel();
    app.exit(if inst.installed() { 0 } else { 1 });
}

fn silent(args: &[String]) -> i32 {
    let value = |flag: &str| {
        args.iter()
            .position(|a| a == flag)
            .and_then(|i| args.get(i + 1))
    };
    let opts = Options {
        dir: value("--dir")
            .map(PathBuf::from)
            .unwrap_or_else(system::default_dir),
        desktop: !args.iter().any(|a| a == "--no-desktop"),
        models: !args.iter().any(|a| a == "--skip-models"),
    };
    match installer().run(&opts, &|_| {}) {
        Ok(()) => 0,
        Err(Stop::Cancelled) => 1,
        Err(Stop::Failed(e)) => {
            eprintln!("{e}");
            1
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--silent") {
        std::process::exit(silent(&args));
    }
    let done = Arc::new(AtomicBool::new(false));
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(installer())
        .invoke_handler(tauri::generate_handler![
            setup_info,
            setup_free_space,
            setup_pick_folder,
            setup_install,
            setup_cancel,
            setup_launch,
            setup_autostart,
            setup_quit
        ])
        .build(tauri::generate_context!())
        .expect("installer window");
    let flag = done.clone();
    app.run(move |handle, event| {
        // Closing the window any other way than setup_quit (Alt+F4) also stops
        // a running installation and reports whether Crosstalk is installed.
        if let tauri::RunEvent::ExitRequested {
            code: None, api, ..
        } = event
        {
            if !flag.swap(true, Ordering::SeqCst) {
                api.prevent_exit();
                let inst = handle.state::<Arc<Installer>>();
                inst.cancel();
                handle.exit(if inst.installed() { 0 } else { 1 });
            }
        }
    });
}
