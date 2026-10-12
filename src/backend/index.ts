// Bridge between the React UI and the Rust core (src-tauri).
//
// The UI always runs inside the Tauri desktop app in production. These helpers also
// degrade to harmless fallbacks when the frontend is loaded outside Tauri (unit tests,
// `npm run dev` for quick styling work), so components never have to check themselves.

import { invoke, isTauri as tauriPresent } from '@tauri-apps/api/core';
import { emitTo, listen, type UnlistenFn } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';

export const isTauri: boolean = tauriPresent();

export interface AppInfo {
  name: string;
  version: string;
  target: string;
}

export const FALLBACK_INFO: AppInfo = { name: 'Crosstalk', version: '0.2.0', target: 'windows-x86_64' };

export async function getAppInfo(): Promise<AppInfo> {
  if (!isTauri) return FALLBACK_INFO;
  try {
    return await invoke<AppInfo>('app_info');
  } catch {
    return FALLBACK_INFO;
  }
}

/** Custom title-bar buttons (the main window is frameless to match the design). */
export const windowControls = {
  minimize: () => (isTauri ? getCurrentWindow().minimize() : Promise.resolve()),
  toggleMaximize: () => (isTauri ? getCurrentWindow().toggleMaximize() : Promise.resolve()),
  /** Hides to the notification area; the app keeps running (see src-tauri/src/lib.rs). */
  close: () => (isTauri ? getCurrentWindow().close() : Promise.resolve()),
};

// ---------- actions from global shortcuts and the tray menu ----------

export type NativeAction = 'overlay' | 'pause' | 'emergency' | 'session';

const noop: UnlistenFn = () => {};

export async function onNativeAction(cb: (a: NativeAction) => void): Promise<UnlistenFn> {
  if (!isTauri) return noop;
  return listen<NativeAction>('crosstalk://action', (e) => cb(e.payload));
}

export interface ShortcutStatus {
  keys: string;
  registered: boolean;
  error: string | null;
}

export async function getShortcutStatus(): Promise<ShortcutStatus[] | null> {
  if (!isTauri) return null;
  try {
    return await invoke<ShortcutStatus[]>('shortcut_status');
  } catch {
    return null;
  }
}

// ---------- in-game overlay window ----------

export const OVERLAY_STATE_EVENT = 'overlay://state';
export const OVERLAY_READY_EVENT = 'overlay://ready';

export interface OverlayWindowConfig {
  visible: boolean;
  clickThrough: boolean;
  corner: 'tl' | 'tr' | 'bl' | 'br' | null;
}

export const overlayWindow = {
  configure: (config: OverlayWindowConfig) => (isTauri ? invoke<void>('overlay_configure', { config }) : Promise.resolve()),
  /** Main window → overlay window: what to draw. */
  pushState: <T>(state: T) => (isTauri ? emitTo('overlay', OVERLAY_STATE_EVENT, state) : Promise.resolve()),
  onState: <T>(cb: (s: T) => void) => (isTauri ? listen<T>(OVERLAY_STATE_EVENT, (e) => cb(e.payload)) : Promise.resolve(noop)),
  /** Overlay window → main window: request the current state after loading. */
  announceReady: () => (isTauri ? emitTo('main', OVERLAY_READY_EVENT) : Promise.resolve()),
  onReady: (cb: () => void) => (isTauri ? listen(OVERLAY_READY_EVENT, cb) : Promise.resolve(noop)),
};

// ---------- launch with Windows ----------

export const autostart = {
  isEnabled: async (): Promise<boolean> => {
    if (!isTauri) return false;
    const { isEnabled } = await import('@tauri-apps/plugin-autostart');
    return isEnabled();
  },
  set: async (on: boolean): Promise<void> => {
    if (!isTauri) return;
    const { enable, disable } = await import('@tauri-apps/plugin-autostart');
    await (on ? enable() : disable());
  },
};

// ---------- audio capture (src-tauri/src/audio) and CS2 detection (src-tauri/src/game) ----------

export type CaptureMode = 'auto' | 'process' | 'device';
export type CaptureStateName = 'idle' | 'starting' | 'capturing' | 'error';
export type StopReason = 'user' | 'emergency' | 'cs2Exited' | 'restart';

export type SourceInfo = { kind: 'process'; pid: number; processName: string } | { kind: 'device'; id: string; name: string };

export interface StreamFormat {
  sampleRate: number;
  channels: number;
  sampleFormat: 'f32' | 'i16' | 'i24' | 'i32';
}

export interface CaptureStatus {
  state: CaptureStateName;
  mode: CaptureMode | null;
  source: SourceInfo | null;
  format: StreamFormat | null;
  /** Set when Auto mode fell back to the output device. */
  fallbackReason: string | null;
  error: { code: string; message: string } | null;
  stopReason: StopReason | null;
}

export interface LevelReading {
  rms: number;
  peak: number;
  rmsDb: number;
  peakDb: number;
}

export interface AudioDevice {
  id: string;
  name: string;
  isDefault: boolean;
}

export interface PlatformInfo {
  windowsBuild: number | null;
  processLoopbackDocumented: boolean | null;
}

export interface Cs2Status {
  running: boolean;
  pid: number | null;
}

export const IDLE_CAPTURE: CaptureStatus = { state: 'idle', mode: null, source: null, format: null, fallbackReason: null, error: null, stopReason: null };

const unsupported = (): CaptureStatus => ({
  ...IDLE_CAPTURE,
  state: 'error',
  error: { code: 'unsupported', message: 'La capture audio n’est disponible que dans l’application de bureau Windows.' },
});

export const audioCapture = {
  start: (mode: CaptureMode, deviceId: string | null): Promise<CaptureStatus> =>
    isTauri ? invoke<CaptureStatus>('audio_start', { mode, deviceId }) : Promise.resolve(unsupported()),
  stop: (): Promise<CaptureStatus> => (isTauri ? invoke<CaptureStatus>('audio_stop') : Promise.resolve(IDLE_CAPTURE)),
  status: (): Promise<CaptureStatus> => (isTauri ? invoke<CaptureStatus>('audio_status') : Promise.resolve(IDLE_CAPTURE)),
  devices: (): Promise<AudioDevice[]> => (isTauri ? invoke<AudioDevice[]>('audio_devices') : Promise.resolve([])),
  platform: (): Promise<PlatformInfo> =>
    isTauri ? invoke<PlatformInfo>('audio_platform') : Promise.resolve({ windowsBuild: null, processLoopbackDocumented: null }),
  onStatus: (cb: (s: CaptureStatus) => void) => (isTauri ? listen<CaptureStatus>('audio://status', (e) => cb(e.payload)) : Promise.resolve(noop)),
  onLevel: (cb: (l: LevelReading) => void) => (isTauri ? listen<LevelReading>('audio://level', (e) => cb(e.payload)) : Promise.resolve(noop)),
};

export const cs2 = {
  status: (): Promise<Cs2Status> => (isTauri ? invoke<Cs2Status>('cs2_status') : Promise.resolve({ running: false, pid: null })),
  onStatus: (cb: (s: Cs2Status) => void) => (isTauri ? listen<Cs2Status>('cs2://status', (e) => cb(e.payload)) : Promise.resolve(noop)),
};

// ---------- speech recognition (src-tauri/src/speech) ----------

export type SpeechLang = 'ru' | 'en';
export type EngineState = 'missing' | 'loading' | 'ready' | 'error';
export type RejectReason =
  | 'short'
  | 'transient'
  | 'stale'
  | 'overflow'
  | 'empty'
  | 'noSpeech'
  | 'lowConfidence'
  | 'hallucination'
  | 'repetition'
  | 'wrongScript';

export interface SpeechConfig {
  model: string;
  languages: SpeechLang[];
  /** 0.2–0.9. */
  vadThreshold: number;
  glossary: string[];
}

export interface SpeechStatus {
  state: EngineState;
  config: SpeechConfig;
  modelPath: string | null;
  vadReady: boolean;
  modelsDir: string;
  error: string | null;
  loadMs: number | null;
  backend: string;
  threads: number;
}

/** A transcription in the speaker's language (not a translation). */
export interface Transcript {
  id: number;
  text: string;
  lang: SpeechLang;
  langProb: number;
  audioMs: number;
  latencyMs: number;
  asrMs: number;
  avgLogprob: number;
  noSpeechProb: number;
  model: string;
}

export interface SpeechStats {
  segments: number;
  transcribed: number;
  rejected: Partial<Record<RejectReason, number>>;
  droppedBlocks: number;
  latencyP50Ms: number | null;
  latencyP95Ms: number | null;
  lastRtf: number | null;
  cpuPercent: number | null;
}

export interface VoiceActivity {
  speaking: boolean;
  transcribing: boolean;
}

export interface ModelInfo {
  id: string;
  file: string;
  sizeMb: number;
  note: string;
  installed: boolean;
  url: string;
}

export const speech = {
  status: (): Promise<SpeechStatus | null> => (isTauri ? invoke<SpeechStatus>('speech_status') : Promise.resolve(null)),
  stats: (): Promise<SpeechStats | null> => (isTauri ? invoke<SpeechStats>('speech_stats') : Promise.resolve(null)),
  models: (): Promise<ModelInfo[]> => (isTauri ? invoke<ModelInfo[]>('speech_models') : Promise.resolve([])),
  configure: (config: SpeechConfig): Promise<SpeechStatus | null> =>
    isTauri ? invoke<SpeechStatus>('speech_configure', { config }) : Promise.resolve(null),
  rescan: (): Promise<SpeechStatus | null> => (isTauri ? invoke<SpeechStatus>('speech_rescan') : Promise.resolve(null)),
  onStatus: (cb: (s: SpeechStatus) => void) => (isTauri ? listen<SpeechStatus>('speech://status', (e) => cb(e.payload)) : Promise.resolve(noop)),
  onTranscript: (cb: (t: Transcript) => void) => (isTauri ? listen<Transcript>('speech://transcript', (e) => cb(e.payload)) : Promise.resolve(noop)),
  onActivity: (cb: (a: VoiceActivity) => void) => (isTauri ? listen<VoiceActivity>('speech://activity', (e) => cb(e.payload)) : Promise.resolve(noop)),
  onStats: (cb: (s: SpeechStats) => void) => (isTauri ? listen<SpeechStats>('speech://stats', (e) => cb(e.payload)) : Promise.resolve(noop)),
};
