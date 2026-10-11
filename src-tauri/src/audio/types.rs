//! Data types shared by the capture service, the backends and the UI (serialised
//! in camelCase for the Tauri events and commands).

use serde::{Deserialize, Serialize};

/// What the user asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CaptureMode {
    /// Audio of `cs2.exe` only (process loopback), falling back to the output
    /// device when process loopback is unavailable.
    Auto,
    /// Audio of `cs2.exe` only, no fallback.
    Process,
    /// Everything played on an output device (endpoint loopback).
    Device,
}

/// One concrete thing a backend can try to open.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CaptureTarget {
    /// Process loopback on a process tree.
    Process { pid: u32 },
    /// Endpoint loopback; `None` means the default output device.
    Device { id: Option<String> },
}

/// The source actually being captured.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum SourceInfo {
    #[serde(rename_all = "camelCase")]
    Process { pid: u32, process_name: String },
    #[serde(rename_all = "camelCase")]
    Device { id: String, name: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SampleFormat {
    F32,
    I16,
    I24,
    I32,
}

impl SampleFormat {
    pub fn bytes(self) -> usize {
        match self {
            SampleFormat::F32 | SampleFormat::I32 => 4,
            SampleFormat::I24 => 3,
            SampleFormat::I16 => 2,
        }
    }
}

/// Stream format delivered by the backend (before any resampling).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamFormat {
    pub sample_rate: u32,
    pub channels: u16,
    pub sample_format: SampleFormat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum CaptureState {
    Idle,
    Starting,
    Capturing,
    Error,
}

/// Snapshot of the capture, pushed to the UI on every change.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureStatus {
    pub state: CaptureState,
    pub mode: Option<CaptureMode>,
    pub source: Option<SourceInfo>,
    pub format: Option<StreamFormat>,
    /// Set when Auto mode had to fall back to the output device.
    pub fallback_reason: Option<String>,
    pub error: Option<ErrorInfo>,
    /// Why the last capture stopped (user, emergency, CS2 closed…).
    pub stop_reason: Option<StopReason>,
}

impl CaptureStatus {
    pub fn idle() -> Self {
        Self {
            state: CaptureState::Idle,
            mode: None,
            source: None,
            format: None,
            fallback_reason: None,
            error: None,
            stop_reason: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ErrorInfo {
    pub code: &'static str,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum StopReason {
    User,
    Emergency,
    Cs2Exited,
    Restart,
}

/// An output (render) device that can be captured in Device mode.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioDevice {
    pub id: String,
    pub name: String,
    pub is_default: bool,
}

/// Level of the captured signal, sent to the UI about 15 times per second.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LevelReading {
    /// RMS of the mono mix over the window, linear 0..1.
    pub rms: f32,
    /// Absolute peak over the window, linear 0..1.
    pub peak: f32,
    pub rms_db: f32,
    pub peak_db: f32,
}

/// A block of captured audio, interleaved `f32` in -1..1.
pub struct AudioBlock<'a> {
    pub samples: &'a [f32],
    pub channels: u16,
    pub sample_rate: u32,
    /// Capture time of the first frame, in 100 ns units of the
    /// QueryPerformanceCounter clock (0 when unknown).
    pub qpc_100ns: u64,
}

impl AudioBlock<'_> {
    pub fn frames(&self) -> usize {
        self.samples.len() / self.channels.max(1) as usize
    }
}
