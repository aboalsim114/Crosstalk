//! WASAPI backend (Windows only).
//!
//! - **Process loopback**: `ActivateAudioInterfaceAsync` on the virtual
//!   `VAD\Process_Loopback` device with `PROCESS_LOOPBACK_MODE_INCLUDE_TARGET_PROCESS_TREE`:
//!   only what `cs2.exe` (and its children) plays is captured.
//! - **Endpoint loopback**: what an output device plays, all applications mixed.
//!
//! Both are public Windows audio APIs; the game process is never opened.

use std::sync::Arc;

use windows::core::{implement, Interface, GUID, HRESULT, HSTRING, PWSTR};
use windows::Win32::Devices::FunctionDiscovery::PKEY_Device_FriendlyName;
use windows::Win32::Foundation::{
    CloseHandle, E_INVALIDARG, HANDLE, RPC_E_CHANGED_MODE, WAIT_OBJECT_0,
};
use windows::Win32::Media::Audio::{
    eCapture, eCommunications, eConsole, eRender, ActivateAudioInterfaceAsync,
    IActivateAudioInterfaceAsyncOperation, IActivateAudioInterfaceCompletionHandler,
    IActivateAudioInterfaceCompletionHandler_Impl, IAudioCaptureClient, IAudioClient,
    IAudioRenderClient, IMMDevice, IMMDeviceEnumerator, MMDeviceEnumerator,
    AUDCLNT_BUFFERFLAGS_SILENT, AUDCLNT_E_DEVICE_INVALIDATED, AUDCLNT_E_SERVICE_NOT_RUNNING,
    AUDCLNT_E_UNSUPPORTED_FORMAT, AUDCLNT_SHAREMODE_SHARED, AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM,
    AUDCLNT_STREAMFLAGS_EVENTCALLBACK, AUDCLNT_STREAMFLAGS_LOOPBACK,
    AUDCLNT_STREAMFLAGS_SRC_DEFAULT_QUALITY, AUDIOCLIENT_ACTIVATION_PARAMS,
    AUDIOCLIENT_ACTIVATION_PARAMS_0, AUDIOCLIENT_ACTIVATION_TYPE_PROCESS_LOOPBACK,
    AUDIOCLIENT_PROCESS_LOOPBACK_PARAMS, DEVICE_STATE_ACTIVE,
    PROCESS_LOOPBACK_MODE_INCLUDE_TARGET_PROCESS_TREE, VIRTUAL_AUDIO_DEVICE_PROCESS_LOOPBACK,
    WAVEFORMATEX, WAVEFORMATEXTENSIBLE,
};
use windows::Win32::System::Com::StructuredStorage::PROPVARIANT;
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize, CLSCTX_ALL,
    COINIT_MULTITHREADED, STGM_READ,
};
use windows::Win32::System::Threading::{
    AvRevertMmThreadCharacteristics, AvSetMmThreadCharacteristicsW, CreateEventW, SetEvent,
    WaitForSingleObject,
};
use windows::Win32::System::Variant::VT_BLOB;
use windows_core::{IUnknown, Ref};

use super::backend::{CaptureBackend, CaptureStream, Packet};
use super::convert;
use super::error::AudioError;
use super::types::{
    AudioBlock, AudioDevice, CaptureTarget, SampleFormat, SourceInfo, StreamFormat,
};

const KSDATAFORMAT_SUBTYPE_PCM: GUID = GUID::from_u128(0x00000001_0000_0010_8000_00aa00389b71);
const KSDATAFORMAT_SUBTYPE_IEEE_FLOAT: GUID =
    GUID::from_u128(0x00000003_0000_0010_8000_00aa00389b71);
const WAVE_FORMAT_PCM: u16 = 1;
const WAVE_FORMAT_IEEE_FLOAT: u16 = 3;
const WAVE_FORMAT_EXTENSIBLE: u16 = 0xFFFE;
/// HRESULT_FROM_WIN32(ERROR_NOT_FOUND): no default output device.
const E_NOTFOUND: HRESULT = HRESULT(0x8007_0490_u32 as i32);

/// Shared-mode buffer of 200 ms: comfortable for a 20 ms read cadence.
const BUFFER_100NS: i64 = 2_000_000;
const ACTIVATION_TIMEOUT_MS: u32 = 5_000;

pub struct WasapiBackend;

impl CaptureBackend for WasapiBackend {
    fn list_devices(&self) -> Result<Vec<AudioDevice>, AudioError> {
        // Own thread so COM is initialised in a known (MTA) apartment, whatever
        // thread the command runs on.
        std::thread::spawn(list_devices_mta)
            .join()
            .unwrap_or_else(|_| Err(AudioError::Os("énumération interrompue".into())))
    }

    fn open(&self, target: &CaptureTarget) -> Result<Box<dyn CaptureStream>, AudioError> {
        let stream = match target {
            CaptureTarget::Process { pid } => open_process(*pid)?,
            CaptureTarget::Device { id } => open_device(id.as_deref())?,
        };
        Ok(Box::new(stream))
    }
}

pub fn backend() -> Arc<dyn CaptureBackend> {
    Arc::new(WasapiBackend)
}

// ---------- errors ----------

fn os(e: windows::core::Error) -> AudioError {
    let code = e.code();
    if code == AUDCLNT_E_DEVICE_INVALIDATED {
        AudioError::DeviceInvalidated
    } else if code == AUDCLNT_E_SERVICE_NOT_RUNNING {
        AudioError::ServiceNotRunning
    } else {
        AudioError::Os(format!("0x{:08X} {}", code.0 as u32, e.message()))
    }
}

fn hr_text(hr: HRESULT) -> String {
    format!("0x{:08X} {}", hr.0 as u32, hr.message())
}

// ---------- COM apartment ----------

/// Initialises COM (MTA) for the current thread and balances it on drop.
struct Com {
    owned: bool,
}

impl Com {
    fn init() -> Result<Self, AudioError> {
        // SAFETY: standard per-thread COM initialisation.
        let hr = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
        if hr == RPC_E_CHANGED_MODE {
            // Already initialised as STA by someone else: COM is usable, but the
            // matching CoUninitialize is not ours to call.
            return Ok(Self { owned: false });
        }
        hr.ok().map_err(os)?;
        Ok(Self { owned: true })
    }
}

impl Drop for Com {
    fn drop(&mut self) {
        if self.owned {
            // SAFETY: balances the successful CoInitializeEx above.
            unsafe { CoUninitialize() };
        }
    }
}

// ---------- devices ----------

fn enumerator() -> Result<IMMDeviceEnumerator, AudioError> {
    // SAFETY: COM is initialised by the caller.
    unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL) }.map_err(os)
}

/// Id and friendly name of a device.
fn describe(device: &IMMDevice) -> Result<(String, String), AudioError> {
    // SAFETY: COM calls on a live device; the id string is freed with CoTaskMemFree.
    unsafe {
        let raw: PWSTR = device.GetId().map_err(os)?;
        let id = raw.to_string().unwrap_or_default();
        CoTaskMemFree(Some(raw.0 as *const _));
        let name = device
            .OpenPropertyStore(STGM_READ)
            .and_then(|store| store.GetValue(&PKEY_Device_FriendlyName))
            .map(|v| v.to_string())
            .unwrap_or_else(|_| id.clone());
        Ok((id, name))
    }
}

fn list_devices_mta() -> Result<Vec<AudioDevice>, AudioError> {
    list_endpoints_mta(false)
}

/// Active endpoints: outputs, or microphones when `capture`.
fn list_endpoints_mta(capture: bool) -> Result<Vec<AudioDevice>, AudioError> {
    let _com = Com::init()?;
    let en = enumerator()?;
    let (flow, role) = if capture {
        (eCapture, eCommunications)
    } else {
        (eRender, eConsole)
    };
    // SAFETY: COM calls on live interfaces.
    unsafe {
        let default_id = en
            .GetDefaultAudioEndpoint(flow, role)
            .ok()
            .and_then(|d| describe(&d).ok())
            .map(|(id, _)| id);
        let collection = en
            .EnumAudioEndpoints(flow, DEVICE_STATE_ACTIVE)
            .map_err(os)?;
        let count = collection.GetCount().map_err(os)?;
        let mut out = Vec::with_capacity(count as usize);
        for i in 0..count {
            let device = collection.Item(i).map_err(os)?;
            let (id, name) = describe(&device)?;
            out.push(AudioDevice {
                is_default: default_id.as_deref() == Some(id.as_str()),
                id,
                name,
            });
        }
        Ok(out)
    }
}

// ---------- formats ----------

/// Reads a (possibly extensible) wave format. The structs are packed, so they
/// are copied out with unaligned reads.
///
/// # Safety
/// `p` must point to a valid WAVEFORMATEX, followed by the extensible part
/// when `wFormatTag` is WAVE_FORMAT_EXTENSIBLE.
unsafe fn parse_format(p: *const WAVEFORMATEX) -> Result<StreamFormat, AudioError> {
    let wfx = std::ptr::read_unaligned(p);
    let tag = { wfx.wFormatTag };
    let bits = { wfx.wBitsPerSample };
    let (valid_bits, float_sub) = if tag == WAVE_FORMAT_EXTENSIBLE {
        let ext = std::ptr::read_unaligned(p as *const WAVEFORMATEXTENSIBLE);
        let sub = { ext.SubFormat };
        let valid = ext.Samples.wValidBitsPerSample;
        let float = if sub == KSDATAFORMAT_SUBTYPE_IEEE_FLOAT {
            Some(true)
        } else if sub == KSDATAFORMAT_SUBTYPE_PCM {
            Some(false)
        } else {
            None
        };
        (Some(valid), float)
    } else {
        (None, None)
    };
    let sample_format = convert::classify(tag, bits, valid_bits, float_sub)
        .ok_or_else(|| AudioError::FormatUnsupported(format!("format 0x{tag:04X}, {bits} bits")))?;
    Ok(StreamFormat {
        sample_rate: { wfx.nSamplesPerSec },
        channels: { wfx.nChannels },
        sample_format,
    })
}

fn wave_format(rate: u32, channels: u16, format: SampleFormat) -> WAVEFORMATEX {
    let bits = (format.bytes() * 8) as u16;
    let block = channels * bits / 8;
    WAVEFORMATEX {
        wFormatTag: if format == SampleFormat::F32 {
            WAVE_FORMAT_IEEE_FLOAT
        } else {
            WAVE_FORMAT_PCM
        },
        nChannels: channels,
        nSamplesPerSec: rate,
        nAvgBytesPerSec: rate * block as u32,
        nBlockAlign: block,
        wBitsPerSample: bits,
        cbSize: 0,
    }
}

// ---------- opening ----------

fn open_device(id: Option<&str>) -> Result<WasapiStream, AudioError> {
    let com = Com::init()?;
    let en = enumerator()?;
    // SAFETY: COM calls on live interfaces; the mix format is freed after use.
    unsafe {
        let device = match id {
            Some(id) => en
                .GetDevice(&HSTRING::from(id))
                .map_err(|_| AudioError::DeviceNotFound(id.to_owned()))?,
            None => en.GetDefaultAudioEndpoint(eRender, eConsole).map_err(|e| {
                if e.code() == E_NOTFOUND {
                    AudioError::NoDevice
                } else {
                    os(e)
                }
            })?,
        };
        let (dev_id, name) = describe(&device)?;
        let client: IAudioClient = device.Activate(CLSCTX_ALL, None).map_err(os)?;
        let mix = client.GetMixFormat().map_err(os)?;
        let parsed = parse_format(mix);
        let init = parsed.as_ref().ok().map(|_| {
            client.Initialize(
                AUDCLNT_SHAREMODE_SHARED,
                AUDCLNT_STREAMFLAGS_LOOPBACK | AUDCLNT_STREAMFLAGS_EVENTCALLBACK,
                BUFFER_100NS,
                0,
                mix,
                None,
            )
        });
        CoTaskMemFree(Some(mix as *const _));
        let format = parsed?;
        if let Some(r) = init {
            r.map_err(os)?;
        }
        WasapiStream::start(com, client, format, SourceInfo::Device { id: dev_id, name })
    }
}

/// Outputs (`capture` false) or microphones, on their own MTA thread.
pub fn list_endpoints(capture: bool) -> Result<Vec<AudioDevice>, AudioError> {
    std::thread::spawn(move || list_endpoints_mta(capture))
        .join()
        .unwrap_or_else(|_| Err(AudioError::Os("énumération interrompue".into())))
}

/// A microphone (the default communications one when `id` is None), in
/// shared event-driven mode (no loopback). Only opened while the user
/// dictates a reply, or while the AI voice routes it to the virtual cable.
pub fn open_microphone(id: Option<&str>) -> Result<Box<dyn CaptureStream>, AudioError> {
    let com = Com::init()?;
    let en = enumerator()?;
    // SAFETY: COM calls on live interfaces; the mix format is freed after use.
    unsafe {
        let device = match id {
            Some(id) => en
                .GetDevice(&HSTRING::from(id))
                .map_err(|_| AudioError::DeviceNotFound(id.to_owned()))?,
            None => en
                .GetDefaultAudioEndpoint(eCapture, eCommunications)
                .map_err(|e| {
                    if e.code() == E_NOTFOUND {
                        AudioError::NoMicrophone
                    } else {
                        os(e)
                    }
                })?,
        };
        let (dev_id, name) = describe(&device)?;
        let client: IAudioClient = device.Activate(CLSCTX_ALL, None).map_err(os)?;
        let mix = client.GetMixFormat().map_err(os)?;
        let parsed = parse_format(mix);
        let init = parsed.as_ref().ok().map(|_| {
            client.Initialize(
                AUDCLNT_SHAREMODE_SHARED,
                AUDCLNT_STREAMFLAGS_EVENTCALLBACK,
                BUFFER_100NS,
                0,
                mix,
                None,
            )
        });
        CoTaskMemFree(Some(mix as *const _));
        let format = parsed?;
        if let Some(r) = init {
            r.map_err(os)?;
        }
        let stream =
            WasapiStream::start(com, client, format, SourceInfo::Device { id: dev_id, name })?;
        Ok(Box::new(stream))
    }
}

/// Activation parameters wrapped in a PROPVARIANT of type VT_BLOB. Declared
/// with the exact x64 PROPVARIANT layout (vt, 3 reserved words, then BLOB) so
/// no PROPVARIANT ever owns the borrowed parameters or tries to free them.
#[repr(C)]
struct BlobPropVariant {
    vt: u16,
    reserved: [u16; 3],
    cb_size: u32,
    p_blob_data: *const u8,
}

#[implement(IActivateAudioInterfaceCompletionHandler)]
struct ActivationDone {
    /// Raw value of the manual-reset event to signal.
    event: usize,
}

impl IActivateAudioInterfaceCompletionHandler_Impl for ActivationDone_Impl {
    fn ActivateCompleted(
        &self,
        _operation: Ref<'_, IActivateAudioInterfaceAsyncOperation>,
    ) -> windows::core::Result<()> {
        // SAFETY: the event outlives the activation: closed after a completed
        // wait, never closed (leaked) when the wait timed out.
        unsafe { SetEvent(HANDLE(self.event as *mut _)) }
    }
}

fn open_process(pid: u32) -> Result<WasapiStream, AudioError> {
    let com = Com::init()?;
    let unavailable = |what: String| AudioError::ProcessLoopbackUnavailable(what);
    // On the heap: if the activation times out, Windows may still read them
    // and signal the event later, so they are then leaked instead of freed.
    let params = Box::new(AUDIOCLIENT_ACTIVATION_PARAMS {
        ActivationType: AUDIOCLIENT_ACTIVATION_TYPE_PROCESS_LOOPBACK,
        Anonymous: AUDIOCLIENT_ACTIVATION_PARAMS_0 {
            ProcessLoopbackParams: AUDIOCLIENT_PROCESS_LOOPBACK_PARAMS {
                TargetProcessId: pid,
                ProcessLoopbackMode: PROCESS_LOOPBACK_MODE_INCLUDE_TARGET_PROCESS_TREE,
            },
        },
    });
    let blob = Box::new(BlobPropVariant {
        vt: VT_BLOB.0,
        reserved: [0; 3],
        cb_size: std::mem::size_of::<AUDIOCLIENT_ACTIVATION_PARAMS>() as u32,
        p_blob_data: &*params as *const _ as *const u8,
    });

    // SAFETY: the event, `params` and `blob` outlive the asynchronous
    // activation: freed after a completed wait, leaked after a timeout.
    unsafe {
        let done = CreateEventW(None, true, false, None).map_err(os)?;
        let handler: IActivateAudioInterfaceCompletionHandler = ActivationDone {
            event: done.0 as usize,
        }
        .into();
        let op = ActivateAudioInterfaceAsync(
            VIRTUAL_AUDIO_DEVICE_PROCESS_LOOPBACK,
            &IAudioClient::IID,
            Some(&*blob as *const BlobPropVariant as *const PROPVARIANT),
            &handler,
        );
        let op = match op {
            Ok(op) => op,
            Err(e) => {
                let _ = CloseHandle(done);
                return Err(unavailable(hr_text(e.code())));
            }
        };
        let waited = WaitForSingleObject(done, ACTIVATION_TIMEOUT_MS);
        if waited != WAIT_OBJECT_0 {
            // The activation may still complete: its handler signals `done`
            // and Windows may read the parameters. Keep all three alive
            // (a few bytes and one handle, only when this rare timeout happens).
            std::mem::forget(handler);
            Box::leak(params);
            Box::leak(blob);
            return Err(AudioError::Timeout(
                "activation de la capture par processus",
            ));
        }
        let _ = CloseHandle(done);
        let mut hr = HRESULT(0);
        let mut unknown: Option<IUnknown> = None;
        op.GetActivateResult(&mut hr, &mut unknown)
            .map_err(|e| unavailable(hr_text(e.code())))?;
        hr.ok().map_err(|e| unavailable(hr_text(e.code())))?;
        let client: IAudioClient = unknown
            .ok_or_else(|| unavailable("aucune interface retournée".into()))?
            .cast()
            .map_err(os)?;

        // Process loopback has no mix format: we choose one. Windows converts.
        let flags = AUDCLNT_STREAMFLAGS_LOOPBACK
            | AUDCLNT_STREAMFLAGS_EVENTCALLBACK
            | AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM
            | AUDCLNT_STREAMFLAGS_SRC_DEFAULT_QUALITY;
        let mut last = None;
        for sf in [SampleFormat::F32, SampleFormat::I16] {
            let wfx = wave_format(48_000, 2, sf);
            match client.Initialize(AUDCLNT_SHAREMODE_SHARED, flags, BUFFER_100NS, 0, &wfx, None) {
                Ok(()) => {
                    let name = crate::game::process::process_name(pid)
                        .unwrap_or_else(|| crate::game::process::CS2_EXE.to_owned());
                    return WasapiStream::start(
                        com,
                        client,
                        StreamFormat {
                            sample_rate: 48_000,
                            channels: 2,
                            sample_format: sf,
                        },
                        SourceInfo::Process {
                            pid,
                            process_name: name,
                        },
                    );
                }
                Err(e) if e.code() == AUDCLNT_E_UNSUPPORTED_FORMAT || e.code() == E_INVALIDARG => {
                    last = Some(e);
                }
                Err(e) => return Err(unavailable(hr_text(e.code()))),
            }
        }
        Err(AudioError::FormatUnsupported(
            last.map(|e| hr_text(e.code())).unwrap_or_default(),
        ))
    }
}

// ---------- stream ----------

struct WasapiStream {
    client: IAudioClient,
    capture: IAudioCaptureClient,
    event: HANDLE,
    mmcss: Option<HANDLE>,
    format: StreamFormat,
    source: SourceInfo,
    block_align: usize,
    buf: Vec<f32>,
    /// Declared last: COM is uninitialised after the interfaces are released.
    _com: Com,
}

impl WasapiStream {
    /// # Safety
    /// `client` must be initialised in event-driven shared mode.
    unsafe fn start(
        com: Com,
        client: IAudioClient,
        format: StreamFormat,
        source: SourceInfo,
    ) -> Result<Self, AudioError> {
        let event = CreateEventW(None, false, false, None).map_err(os)?;
        let setup = (|| {
            client.SetEventHandle(event)?;
            let capture: IAudioCaptureClient = client.GetService()?;
            client.Start()?;
            Ok::<_, windows::core::Error>(capture)
        })();
        let capture = match setup {
            Ok(c) => c,
            Err(e) => {
                let _ = CloseHandle(event);
                return Err(os(e));
            }
        };
        // Real-time scheduling class for the capture thread; best effort.
        let mut task = 0u32;
        let mmcss = AvSetMmThreadCharacteristicsW(windows::core::w!("Pro Audio"), &mut task).ok();
        Ok(Self {
            client,
            capture,
            event,
            mmcss,
            block_align: format.channels as usize * format.sample_format.bytes(),
            format,
            source,
            buf: Vec::new(),
            _com: com,
        })
    }
}

impl CaptureStream for WasapiStream {
    fn format(&self) -> StreamFormat {
        self.format
    }

    fn source(&self) -> SourceInfo {
        self.source.clone()
    }

    fn read(
        &mut self,
        timeout_ms: u32,
        on_packet: &mut dyn FnMut(Packet<'_>),
    ) -> Result<(), AudioError> {
        // SAFETY: COM calls on live interfaces; the packet buffer is copied out
        // before ReleaseBuffer.
        unsafe {
            // Woken by WASAPI or by the timeout; packets are drained either way,
            // which also covers drivers that do not signal loopback events.
            WaitForSingleObject(self.event, timeout_ms);
            loop {
                let pending = self.capture.GetNextPacketSize().map_err(os)?;
                if pending == 0 {
                    return Ok(());
                }
                let mut data: *mut u8 = std::ptr::null_mut();
                let mut frames = 0u32;
                let mut flags = 0u32;
                let mut qpc = 0u64;
                self.capture
                    .GetBuffer(&mut data, &mut frames, &mut flags, None, Some(&mut qpc))
                    .map_err(os)?;
                if flags & AUDCLNT_BUFFERFLAGS_SILENT.0 as u32 != 0 || data.is_null() {
                    on_packet(Packet::Silence {
                        frames: frames as usize,
                        qpc_100ns: qpc,
                    });
                } else {
                    let bytes =
                        std::slice::from_raw_parts(data, frames as usize * self.block_align);
                    self.buf.clear();
                    convert::to_f32(bytes, self.format.sample_format, &mut self.buf);
                    on_packet(Packet::Audio(AudioBlock {
                        samples: &self.buf,
                        channels: self.format.channels,
                        sample_rate: self.format.sample_rate,
                        qpc_100ns: qpc,
                    }));
                }
                self.capture.ReleaseBuffer(frames).map_err(os)?;
            }
        }
    }
}

impl Drop for WasapiStream {
    fn drop(&mut self) {
        // SAFETY: releasing resources this stream owns.
        unsafe {
            let _ = self.client.Stop();
            let _ = CloseHandle(self.event);
            if let Some(h) = self.mmcss.take() {
                let _ = AvRevertMmThreadCharacteristics(h);
            }
        }
    }
}

// ---------- output (AI voice) ----------

/// Output buffer of 40 ms: the microphone passed to the virtual cable stays
/// close to real time.
const RENDER_BUFFER_100NS: i64 = 400_000;

/// An output device in shared event-driven mode, fed with mono samples.
pub struct RenderStream {
    client: IAudioClient,
    render: IAudioRenderClient,
    event: HANDLE,
    format: StreamFormat,
    frames: u32,
    name: String,
    /// Frames still waiting in the device buffer after the last write.
    queued: u32,
    /// Declared last: COM is uninitialised after the interfaces are released.
    _com: Com,
}

/// Opens an output (the default one when `id` is None).
pub fn open_render(id: Option<&str>) -> Result<RenderStream, AudioError> {
    let com = Com::init()?;
    let en = enumerator()?;
    // SAFETY: COM calls on live interfaces; the mix format is freed after use.
    unsafe {
        let device = match id {
            Some(id) => en
                .GetDevice(&HSTRING::from(id))
                .map_err(|_| AudioError::DeviceNotFound(id.to_owned()))?,
            None => en.GetDefaultAudioEndpoint(eRender, eConsole).map_err(|e| {
                if e.code() == E_NOTFOUND {
                    AudioError::NoDevice
                } else {
                    os(e)
                }
            })?,
        };
        let (_, name) = describe(&device)?;
        let client: IAudioClient = device.Activate(CLSCTX_ALL, None).map_err(os)?;
        let mix = client.GetMixFormat().map_err(os)?;
        let parsed = parse_format(mix);
        let init = parsed.as_ref().ok().map(|_| {
            client.Initialize(
                AUDCLNT_SHAREMODE_SHARED,
                AUDCLNT_STREAMFLAGS_EVENTCALLBACK,
                RENDER_BUFFER_100NS,
                0,
                mix,
                None,
            )
        });
        CoTaskMemFree(Some(mix as *const _));
        let format = parsed?;
        if !matches!(format.sample_format, SampleFormat::F32 | SampleFormat::I16) {
            return Err(AudioError::FormatUnsupported(format!(
                "{:?}",
                format.sample_format
            )));
        }
        if let Some(r) = init {
            r.map_err(os)?;
        }
        let event = CreateEventW(None, false, false, None).map_err(os)?;
        let setup = (|| {
            client.SetEventHandle(event)?;
            let frames = client.GetBufferSize()?;
            let render: IAudioRenderClient = client.GetService()?;
            client.Start()?;
            Ok::<_, windows::core::Error>((frames, render))
        })();
        let (frames, render) = match setup {
            Ok(v) => v,
            Err(e) => {
                let _ = CloseHandle(event);
                return Err(os(e));
            }
        };
        Ok(RenderStream {
            client,
            render,
            event,
            format,
            frames,
            name,
            queued: 0,
            _com: com,
        })
    }
}

impl RenderStream {
    pub fn format(&self) -> StreamFormat {
        self.format
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    /// Frames waiting in the device buffer just after the last write (not yet heard).
    pub fn queued(&self) -> u32 {
        self.queued
    }

    /// Waits up to `timeout_ms` for room, then asks `fill` for that many mono
    /// samples (it must return exactly `n`) and writes them on every channel.
    pub fn write(
        &mut self,
        timeout_ms: u32,
        fill: &mut dyn FnMut(usize) -> Vec<f32>,
    ) -> Result<(), AudioError> {
        // SAFETY: COM calls on live interfaces; the buffer is written within
        // the `n` frames WASAPI gave us, then released.
        unsafe {
            WaitForSingleObject(self.event, timeout_ms);
            let padding = self.client.GetCurrentPadding().map_err(os)?;
            let n = self.frames.saturating_sub(padding) as usize;
            self.queued = padding;
            if n == 0 {
                return Ok(());
            }
            let mut mono = fill(n);
            mono.resize(n, 0.0);
            let ch = self.format.channels as usize;
            let data = self.render.GetBuffer(n as u32).map_err(os)?;
            match self.format.sample_format {
                SampleFormat::F32 => {
                    let out = std::slice::from_raw_parts_mut(data as *mut f32, n * ch);
                    for (frame, s) in out.chunks_exact_mut(ch).zip(&mono) {
                        frame.fill(*s);
                    }
                }
                _ => {
                    let out = std::slice::from_raw_parts_mut(data as *mut i16, n * ch);
                    for (frame, s) in out.chunks_exact_mut(ch).zip(&mono) {
                        frame.fill((s.clamp(-1.0, 1.0) * 32767.0) as i16);
                    }
                }
            }
            self.render.ReleaseBuffer(n as u32, 0).map_err(os)?;
            self.queued = padding + n as u32;
            Ok(())
        }
    }
}

impl Drop for RenderStream {
    fn drop(&mut self) {
        // SAFETY: releasing resources this stream owns.
        unsafe {
            let _ = self.client.Stop();
            let _ = CloseHandle(self.event);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blob_propvariant_matches_the_propvariant_layout() {
        assert_eq!(
            std::mem::size_of::<BlobPropVariant>(),
            std::mem::size_of::<PROPVARIANT>()
        );
    }

    #[test]
    fn requested_formats_are_consistent() {
        let f = wave_format(48_000, 2, SampleFormat::F32);
        assert_eq!({ f.nBlockAlign }, 8);
        assert_eq!({ f.nAvgBytesPerSec }, 384_000);
        let parsed = unsafe { parse_format(&f) }.unwrap();
        assert_eq!(parsed.sample_format, SampleFormat::F32);
        let p = wave_format(48_000, 2, SampleFormat::I16);
        assert_eq!(
            unsafe { parse_format(&p) }.unwrap().sample_format,
            SampleFormat::I16
        );
    }

    /// Exercises the real device enumeration. CI runners may have no audio
    /// device: an empty list is fine, a COM failure is not.
    #[test]
    fn device_enumeration_works_on_this_machine() {
        match WasapiBackend.list_devices() {
            Ok(devices) => {
                assert!(devices.iter().filter(|d| d.is_default).count() <= 1);
                println!("{} output device(s)", devices.len());
            }
            Err(AudioError::NoDevice) => {}
            Err(e) => panic!("enumeration failed: {e:?}"),
        }
    }

    /// Activates process loopback on this test process. Must return cleanly,
    /// whether or not the machine supports it (CI runners may lack audio).
    #[test]
    fn process_loopback_activation_returns_cleanly() {
        let result = WasapiBackend.open(&CaptureTarget::Process {
            pid: std::process::id(),
        });
        match result {
            Ok(mut stream) => {
                assert_eq!(stream.format().sample_rate, 48_000);
                let mut packets = 0;
                stream.read(20, &mut |_| packets += 1).unwrap();
                println!("process loopback opened, {packets} packet(s)");
            }
            Err(e) => println!("process loopback unavailable here: {e}"),
        }
    }
}
