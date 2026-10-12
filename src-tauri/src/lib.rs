//! Crosstalk desktop core.
//!
//! The React UI (../src) is embedded in this binary and rendered by WebView2.
//! All native features live here, behind Tauri commands, events and plugins:
//!
//! - `audio`     WASAPI capture of cs2.exe (process loopback, device fallback), levels
//! - `speech`    Silero VAD + whisper.cpp transcription of the captured voices (ru, en)
//! - `game`      CS2 process detection (names and PIDs only, the game is never opened)
//! - `tray`      notification-area icon; the app keeps running when the window is closed
//! - `shortcuts` system-wide Ctrl+Alt+F8 / F9 / F10
//! - `overlay`   transparent, always-on-top, click-through subtitle window
//! - autostart   optional launch with Windows (`--minimized`, straight to the tray)
//! - `pipeline`  seams for audio capture, speech recognition, translation and CS2

mod actions;
pub mod audio;
mod commands;
pub mod downloads;
pub mod game;
mod overlay;
// Traits and helpers for the upcoming native pipeline; wired in later phases.
#[allow(dead_code)]
mod pipeline;
mod shortcuts;
pub mod speech;
pub mod translation;
mod tray;

use std::sync::Arc;

use tauri::{Emitter, Manager, WindowEvent};
use tauri_plugin_autostart::MacosLauncher;

/// Passed by the autostart entry so Windows login starts Crosstalk in the tray.
pub const MINIMIZED_FLAG: &str = "--minimized";

/// Capture events go to the UI; a capture that stops also purges the speech and
/// translation pipelines (nothing survives a stop, an emergency stop or CS2 closing).
struct CaptureEvents {
    ui: audio::TauriEvents,
    speech: Arc<speech::SpeechService>,
    translation: Arc<translation::TranslationService>,
}

impl audio::AudioEvents for CaptureEvents {
    fn status(&self, status: &audio::CaptureStatus) {
        if status.state != audio::CaptureState::Capturing {
            self.speech.purge();
            self.translation.purge();
        }
        self.ui.status(status);
    }
    fn level(&self, reading: audio::LevelReading) {
        self.ui.level(reading);
    }
}

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
            shortcuts::shortcut_status,
            audio::audio_status,
            audio::audio_platform,
            audio::audio_devices,
            audio::audio_start,
            audio::audio_stop,
            game::cs2_status,
            speech::speech_status,
            speech::speech_stats,
            speech::speech_models,
            speech::speech_configure,
            speech::speech_rescan,
            translation::translation_status,
            translation::translation_configure,
            translation::translation_rescan,
            translation::translation_models,
            translation::translation_test,
            downloads::models_download_status,
            downloads::models_download_start,
            downloads::models_download_cancel
        ])
        .setup(|app| {
            let handle = app.handle();

            // Translation decodes one token at a time: one thread is nearly as
            // fast as four and leaves the CPU to CS2 and Whisper (read by candle).
            if std::env::var_os("RAYON_NUM_THREADS").is_none() {
                std::env::set_var("RAYON_NUM_THREADS", "1");
            }
            app.manage(downloads::Downloader::new(
                Arc::new(downloads::HttpFetcher),
                Arc::new(downloads::TauriDownloadEvents(handle.clone())),
            ));
            let translation = translation::start(handle);
            app.manage(translation.clone());
            let speech = speech::start(handle);
            app.manage(speech.clone());
            let audio = Arc::new(audio::AudioService::new(
                audio::default_backend(),
                Arc::new(CaptureEvents {
                    ui: audio::TauriEvents(handle.clone()),
                    speech: speech.clone(),
                    translation: translation.clone(),
                }),
            ));
            audio.set_sink(Some(Box::new(speech.sink())));
            app.manage(audio.clone());
            let watcher = Arc::new(game::Cs2Watcher::default());
            watcher.poll();
            app.manage(watcher.clone());
            let events = handle.clone();
            watcher.spawn(move |change, status| {
                let _ = events.emit(game::STATUS_EVENT, status);
                match change {
                    game::Cs2Change::Exited(pid) | game::Cs2Change::Restarted { old: pid, .. } => {
                        audio.on_process_exited(pid)
                    }
                    game::Cs2Change::Started(_) => {}
                }
            });

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
