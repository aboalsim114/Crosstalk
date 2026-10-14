//! Crosstalk desktop core.
//!
//! The React UI (../src) is embedded in this binary and rendered by WebView2.
//! All native features live here, behind Tauri commands, events and plugins:
//!
//! - `audio`     WASAPI capture of cs2.exe (process loopback, device fallback), levels
//! - `speech`    Silero VAD + whisper.cpp transcription of the captured voices
//! - `translation` local Opus-MT translation into the user's language
//! - `chat`      CS2 text chat read from console.log
//! - `game`      CS2 process detection (names and PIDs only, the game is never opened)
//! - `tray`      notification-area icon; the app keeps running when the window is closed
//! - `shortcuts` system-wide Ctrl+Alt+F8 / F9 / F10
//! - `overlay`   transparent, always-on-top, click-through subtitle window
//! - autostart   optional launch with Windows (`--minimized`, straight to the tray)
//!
//! See docs/architecture/overview.md for the whole picture.

mod actions;
pub mod audio;
pub mod chat;
mod clipboard;
mod commands;
pub mod cpu;
mod dictation;
mod download_commands;
pub mod downloads;
pub mod game;
mod gsi;
mod history;
mod logs;
mod overlay;
pub mod paths;
mod recap;
mod reply;
mod settings;
mod shortcuts;
mod speaker;
pub mod speech;
mod stream;
pub mod translation;
mod tray;
mod updater;
mod version;
mod wiring;

use std::sync::Arc;

use tauri::{Emitter, Manager, WindowEvent};
use tauri_plugin_autostart::MacosLauncher;

/// Passed by the autostart entry so Windows login starts Crosstalk in the tray.
pub const MINIMIZED_FLAG: &str = "--minimized";

pub fn run() {
    // GPU test in a separate process (speech::gpu_guard): a driver crash
    // there only ends this process, never the app.
    let args: Vec<String> = std::env::args().collect();
    if let Some(i) = args.iter().position(|a| a == "--gpu-probe") {
        std::process::exit(speech::gpu_guard::probe_main(
            args.get(i + 1).map(String::as_str),
        ));
    }
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
            wiring::translation_configure,
            translation::translation_rescan,
            translation::translation_models,
            translation::translation_test,
            download_commands::models_download_status,
            download_commands::models_download_start,
            download_commands::models_download_cancel,
            chat::chat_status,
            chat::chat_configure,
            chat::chat_rescan,
            chat::chat_start,
            chat::chat_stop,
            settings::settings_load,
            settings::settings_save,
            updater::update_status,
            updater::update_check,
            updater::update_download,
            updater::update_cancel,
            updater::update_install,
            history::history_append,
            history::history_list,
            history::history_count,
            history::history_clear,
            history::history_export,
            logs::logs_recent,
            logs::logs_write,
            logs::logs_open_folder,
            logs::diag_report,
            logs::diag_report_issue,
            gsi::gsi_status,
            gsi::gsi_install,
            gsi::gsi_uninstall,
            reply::reply_phrases,
            reply::reply_phrase_text,
            reply::reply_translate,
            reply::reply_models,
            reply::reply_open,
            reply::reply_hide,
            dictation::dictation_toggle,
            dictation::dictation_state,
            stream::stream_status,
            stream::stream_configure,
            stream::stream_push,
            speaker::speaker_status,
            speaker::speaker_configure,
            speaker::speaker_test,
            recap::recap_save,
            recap::recap_reveal,
            tray::ui_set_tray_labels
        ])
        .setup(|app| {
            let handle = app.handle();
            logs::init(logs::default_dir());
            logs::info(
                "app",
                &format!(
                    "Crosstalk {} · build {} · commit {} · Windows {}",
                    env!("CARGO_PKG_VERSION"),
                    version::current_build(),
                    env!("CROSSTALK_COMMIT"),
                    audio::platform::windows_build()
                        .map(|b| b.to_string())
                        .unwrap_or_else(|| "?".into())
                ),
            );
            logs::watch(handle);

            // Translation decodes one token at a time: one thread is nearly as
            // fast as four and leaves the CPU to CS2 and Whisper (read by candle).
            if std::env::var_os("RAYON_NUM_THREADS").is_none() {
                std::env::set_var("RAYON_NUM_THREADS", "1");
            }
            app.manage(downloads::Downloader::new(
                Arc::new(downloads::HttpFetcher),
                Arc::new(download_commands::TauriDownloadEvents(handle.clone())),
            ));
            let translation = translation::start(handle);
            app.manage(translation.clone());
            let speech = wiring::start_speech(handle);
            app.manage(speech.clone());
            let history = history::History::new(history::default_path());
            let _ = history.purge(history::now_ms());
            app.manage(history);
            app.manage(updater::Updater::new(
                Arc::new(downloads::HttpFetcher),
                Arc::new(updater::TauriUpdateEvents(handle.clone())),
                std::env::temp_dir().join("Crosstalk-update"),
                updater::feed_url(),
            ));
            let chat = chat::ChatService::new(
                Arc::new(wiring::ChatBridge(handle.clone())),
                chat::user_rules_file(),
            );
            app.manage(chat.clone());
            let reply = reply::ReplyService::new(paths::models_dir());
            app.manage(reply.clone());
            app.manage(Arc::new(dictation::Dictation::default()));
            app.manage(stream::StreamService::new());
            let speaker = speaker::SpeakerService::new();
            speaker::start(handle, speaker.clone());
            app.manage(speaker);
            let gsi = gsi::GsiService::new(Arc::new(gsi::TauriGsiEvents {
                app: handle.clone(),
                speech: speech.clone(),
                translation: translation.clone(),
                reply,
            }));
            gsi.refresh(
                chat.status()
                    .setup
                    .cs2_dir
                    .map(std::path::PathBuf::from)
                    .as_deref(),
            );
            app.manage(gsi.clone());
            let audio = Arc::new(audio::AudioService::new(
                audio::default_backend(),
                Arc::new(wiring::CaptureEvents {
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
                        gsi.game_closed();
                        audio.on_process_exited(pid)
                    }
                    game::Cs2Change::Started(_) => {}
                }
            });

            tray::create(handle)?;
            shortcuts::register_all(handle);
            overlay::create(handle)?;
            // Two screens: the overlay goes where CS2 is (checked every 3 s).
            let follow = handle.clone();
            std::thread::Builder::new()
                .name("overlay-follow".into())
                .spawn(move || loop {
                    std::thread::sleep(std::time::Duration::from_secs(3));
                    overlay::follow_cs2(&follow);
                })?;
            reply::create_window(handle)?;
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
        .build(tauri::generate_context!())
        .expect("error while running Crosstalk")
        .run(|_, event| {
            if let tauri::RunEvent::Exit = event {
                speech::gpu_guard::clean_exit();
            }
        });
}
