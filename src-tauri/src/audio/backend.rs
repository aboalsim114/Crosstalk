//! Seam between the capture service and the OS audio API (WASAPI on Windows),
//! so the service logic can be tested with a fake backend on any platform.

use super::error::AudioError;
use super::types::{AudioBlock, AudioDevice, CaptureTarget, SourceInfo, StreamFormat};

/// An open capture stream. Lives on the capture thread only (COM objects are
/// not `Send`), which is why the backend opens it from that thread.
pub trait CaptureStream {
    fn format(&self) -> StreamFormat;
    fn source(&self) -> SourceInfo;
    /// Waits up to `timeout_ms` for audio and delivers every available packet.
    /// Silent packets are delivered as `None` with their frame count.
    fn read(
        &mut self,
        timeout_ms: u32,
        on_packet: &mut dyn FnMut(Packet<'_>),
    ) -> Result<(), AudioError>;
}

pub enum Packet<'a> {
    Audio(AudioBlock<'a>),
    Silence { frames: usize, qpc_100ns: u64 },
}

pub trait CaptureBackend: Send + Sync + 'static {
    fn list_devices(&self) -> Result<Vec<AudioDevice>, AudioError>;
    /// Opens `target`. Called on the capture thread.
    fn open(&self, target: &CaptureTarget) -> Result<Box<dyn CaptureStream>, AudioError>;
}
