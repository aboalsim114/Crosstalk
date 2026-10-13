//! Native audio capture of Counter-Strike 2.
//!
//! WASAPI process loopback on `cs2.exe`, with an announced fallback to the
//! loopback of an output device; real level metering for the UI. Only public
//! Windows audio APIs are used.

pub mod backend;
pub mod convert;
pub mod error;
pub mod level;
pub mod plan;
pub mod platform;
pub mod service;
pub mod types;
#[cfg(windows)]
pub mod wasapi;

use std::sync::Arc;

use tauri::{AppHandle, Emitter, State};

use crate::game::Cs2Watcher;
pub use backend::CaptureBackend;
pub use error::AudioError;
pub use service::{AudioEvents, AudioService, BlockSink};
pub use types::*;

pub const STATUS_EVENT: &str = "audio://status";
pub const LEVEL_EVENT: &str = "audio://level";

/// The OS backend: WASAPI on Windows, an "unsupported" backend elsewhere (so
/// the app still builds and runs for UI work on other platforms).
/// The default microphone (reply dictation only, opened on demand).
pub fn open_microphone() -> Result<Box<dyn backend::CaptureStream>, AudioError> {
    #[cfg(windows)]
    {
        wasapi::open_microphone()
    }
    #[cfg(not(windows))]
    {
        Err(AudioError::Unsupported)
    }
}

pub fn default_backend() -> Arc<dyn CaptureBackend> {
    #[cfg(windows)]
    {
        wasapi::backend()
    }
    #[cfg(not(windows))]
    {
        Arc::new(UnsupportedBackend)
    }
}

#[cfg(not(windows))]
pub struct UnsupportedBackend;

#[cfg(not(windows))]
impl CaptureBackend for UnsupportedBackend {
    fn list_devices(&self) -> Result<Vec<AudioDevice>, AudioError> {
        Err(AudioError::Unsupported)
    }
    fn open(&self, _target: &CaptureTarget) -> Result<Box<dyn backend::CaptureStream>, AudioError> {
        Err(AudioError::Unsupported)
    }
}

/// Forwards capture events to every window.
pub struct TauriEvents(pub AppHandle);

impl AudioEvents for TauriEvents {
    fn status(&self, status: &CaptureStatus) {
        let _ = self.0.emit(STATUS_EVENT, status);
    }
    fn level(&self, reading: LevelReading) {
        let _ = self.0.emit(LEVEL_EVENT, reading);
    }
}

// ---------- commands ----------

#[tauri::command]
pub fn audio_status(audio: State<'_, Arc<AudioService>>) -> CaptureStatus {
    audio.status()
}

#[tauri::command]
pub fn audio_platform() -> platform::PlatformInfo {
    platform::info()
}

#[tauri::command]
pub async fn audio_devices(
    audio: State<'_, Arc<AudioService>>,
) -> Result<Vec<AudioDevice>, String> {
    let audio = audio.inner().clone();
    tauri::async_runtime::spawn_blocking(move || audio.devices())
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())
}

/// Starts the capture. `device_id` is used in Device mode and as the fallback
/// device in Auto mode (`None`: default output device).
#[tauri::command]
pub async fn audio_start(
    audio: State<'_, Arc<AudioService>>,
    watcher: State<'_, Arc<Cs2Watcher>>,
    mode: CaptureMode,
    device_id: Option<String>,
) -> Result<CaptureStatus, String> {
    let audio = audio.inner().clone();
    let cs2_pid = watcher.status().pid;
    tauri::async_runtime::spawn_blocking(move || audio.start(mode, device_id, cs2_pid))
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn audio_stop(audio: State<'_, Arc<AudioService>>) -> CaptureStatus {
    audio.stop(StopReason::User)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_serialises_in_camel_case_for_the_ui() {
        let st = CaptureStatus {
            state: CaptureState::Capturing,
            mode: Some(CaptureMode::Auto),
            source: Some(SourceInfo::Process {
                pid: 7,
                process_name: "cs2.exe".into(),
            }),
            format: Some(StreamFormat {
                sample_rate: 48_000,
                channels: 2,
                sample_format: SampleFormat::F32,
            }),
            fallback_reason: None,
            error: None,
            stop_reason: None,
        };
        let json = serde_json::to_value(&st).unwrap();
        assert_eq!(
            json,
            serde_json::json!({
                "state": "capturing",
                "mode": "auto",
                "source": { "kind": "process", "pid": 7, "processName": "cs2.exe" },
                "format": { "sampleRate": 48000, "channels": 2, "sampleFormat": "f32" },
                "fallbackReason": null,
                "error": null,
                "stopReason": null
            })
        );
    }

    #[test]
    fn modes_deserialise_from_the_ui() {
        let m: CaptureMode = serde_json::from_str("\"device\"").unwrap();
        assert_eq!(m, CaptureMode::Device);
    }
}
