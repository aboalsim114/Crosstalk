//! Audio capture (to be implemented with WASAPI).
//!
//! Preferred: process loopback targeting `cs2.exe` (Windows 10 build 20348+ / 11).
//! Fallback: endpoint loopback of the output device (captures every application).

use super::PipelineError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureSource {
    /// Only the audio rendered by one process (e.g. `cs2.exe`).
    Process { pid: u32 },
    /// Everything played on the default output device.
    Device,
    /// A WAV file, to test the chain without the game.
    WavFile,
}

/// Mono 16 kHz PCM frames, the format whisper expects.
pub struct AudioFrame {
    pub samples: Vec<f32>,
}

pub trait AudioCapture: Send {
    fn start(&mut self, source: CaptureSource) -> Result<(), PipelineError>;
    /// Next frame, or `None` when the capture stopped.
    fn next_frame(&mut self) -> Option<AudioFrame>;
    /// Stops capturing and drops any buffered audio (emergency stop).
    fn stop(&mut self);
}

/// First Windows build with process-scoped loopback capture.
pub const PROCESS_LOOPBACK_MIN_BUILD: u32 = 20348;

/// Picks the best capture source available on this Windows build.
pub fn preferred_source(windows_build: u32, cs2_pid: Option<u32>) -> CaptureSource {
    match cs2_pid {
        Some(pid) if windows_build >= PROCESS_LOOPBACK_MIN_BUILD => CaptureSource::Process { pid },
        _ => CaptureSource::Device,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uses_process_loopback_on_recent_builds_only() {
        assert_eq!(
            preferred_source(26100, Some(42)),
            CaptureSource::Process { pid: 42 }
        );
        assert_eq!(preferred_source(19045, Some(42)), CaptureSource::Device);
        assert_eq!(preferred_source(26100, None), CaptureSource::Device);
    }
}
