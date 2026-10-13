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

export type NativeAction = 'overlay' | 'pause' | 'emergency' | 'session' | 'reply';

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
  /** Where Whisper runs; auto = GPU only if measured clearly faster than the CPU. */
  accel: 'cpu' | 'gpu' | 'auto';
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
  /** Utterance id: live (partial) transcripts and the final one share it. */
  id: number;
  /** Live words of a segment still being spoken; the final transcript replaces them. */
  partial: boolean;
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

// ---------- local translation (Opus-MT, no API key) ----------

export interface TranslationTerm {
  source: string;
  /** Imposed French rendering; null keeps the term as spoken. */
  target: string | null;
}
export interface TranslationConfig {
  enabled: boolean;
  glossary: TranslationTerm[];
}
export type PairState = 'missing' | 'loading' | 'ready' | 'error';
export interface PairStatus {
  source: 'ru' | 'en';
  repo: string;
  state: PairState;
  error: string | null;
  loadMs: number | null;
}
export interface TranslationStatus {
  config: TranslationConfig;
  pairs: PairStatus[];
  modelsDir: string;
  queued: number;
}
/** French translation of a transcript (matched by `id`), or why it is missing. */
export interface Translation {
  id: number;
  lang: string;
  original: string;
  translated: string | null;
  error: string | null;
  ms: number;
  queueMs: number;
  cached: boolean;
  glossary: boolean;
  /** Quick translation of a live line, replaced by the final one. */
  partial?: boolean;
}

export const translation = {
  status: (): Promise<TranslationStatus | null> => (isTauri ? invoke<TranslationStatus>('translation_status') : Promise.resolve(null)),
  configure: (config: TranslationConfig): Promise<TranslationStatus | null> =>
    isTauri ? invoke<TranslationStatus>('translation_configure', { config }) : Promise.resolve(null),
  rescan: (): Promise<TranslationStatus | null> => (isTauri ? invoke<TranslationStatus>('translation_rescan') : Promise.resolve(null)),
  test: (lang: string, text: string): Promise<Translation | null> =>
    isTauri ? invoke<Translation>('translation_test', { lang, text }) : Promise.resolve(null),
  onStatus: (cb: (s: TranslationStatus) => void) =>
    isTauri ? listen<TranslationStatus>('translation://status', (e) => cb(e.payload)) : Promise.resolve(noop),
  onResult: (cb: (t: Translation) => void) => (isTauri ? listen<Translation>('translation://result', (e) => cb(e.payload)) : Promise.resolve(noop)),
};

// ---------- first-run model download (src-tauri/src/downloads.rs) ----------

export type DownloadState = 'idle' | 'downloading' | 'done' | 'error' | 'cancelled';
export interface DownloadStatus {
  state: DownloadState;
  current: string | null;
  doneBytes: number;
  totalBytes: number;
  missingFiles: number;
  missingBytes: number;
  error: string | null;
}

export const modelDownload = {
  status: (model: string, translation: boolean): Promise<DownloadStatus | null> =>
    isTauri ? invoke<DownloadStatus>('models_download_status', { model, translation }) : Promise.resolve(null),
  start: (model: string, translation: boolean): Promise<DownloadStatus | null> =>
    isTauri ? invoke<DownloadStatus>('models_download_start', { model, translation }) : Promise.resolve(null),
  cancel: (): Promise<void> => (isTauri ? invoke<void>('models_download_cancel') : Promise.resolve()),
  onProgress: (cb: (s: DownloadStatus) => void) => (isTauri ? listen<DownloadStatus>('models://download', (e) => cb(e.payload)) : Promise.resolve(noop)),
};

export const speech = {
  status: (): Promise<SpeechStatus | null> => (isTauri ? invoke<SpeechStatus>('speech_status') : Promise.resolve(null)),
  stats: (): Promise<SpeechStats | null> => (isTauri ? invoke<SpeechStats>('speech_stats') : Promise.resolve(null)),
  models: (): Promise<ModelInfo[]> => (isTauri ? invoke<ModelInfo[]>('speech_models') : Promise.resolve([])),
  configure: (config: SpeechConfig): Promise<SpeechStatus | null> =>
    isTauri ? invoke<SpeechStatus>('speech_configure', { config }) : Promise.resolve(null),
  rescan: (): Promise<SpeechStatus | null> => (isTauri ? invoke<SpeechStatus>('speech_rescan') : Promise.resolve(null)),
  onStatus: (cb: (s: SpeechStatus) => void) => (isTauri ? listen<SpeechStatus>('speech://status', (e) => cb(e.payload)) : Promise.resolve(noop)),
  onTranscript: (cb: (t: Transcript) => void) => (isTauri ? listen<Transcript>('speech://transcript', (e) => cb(e.payload)) : Promise.resolve(noop)),
  /** The live line of this utterance must go (its segment was finally rejected). */
  onRetract: (cb: (id: number) => void) => (isTauri ? listen<number>('speech://retract', (e) => cb(e.payload)) : Promise.resolve(noop)),
  onActivity: (cb: (a: VoiceActivity) => void) => (isTauri ? listen<VoiceActivity>('speech://activity', (e) => cb(e.payload)) : Promise.resolve(noop)),
  onStats: (cb: (s: SpeechStats) => void) => (isTauri ? listen<SpeechStats>('speech://stats', (e) => cb(e.payload)) : Promise.resolve(noop)),
};

// ---------- written chat of CS2 (src-tauri/src/chat) ----------

export type ChatChannel = 'ALL' | 'CT' | 'T';
export interface ChatMessage {
  id: number;
  channel: ChatChannel;
  sender: string;
  message: string;
  location: string | null;
  dead: boolean;
  /** ru, en, fr, another code, or null when the text gives no clue. */
  lang: string | null;
}
export interface Cs2Setup {
  steamDir: string | null;
  libraries: string[];
  cs2Dir: string | null;
  consoleLog: string | null;
  consoleLogExists: boolean;
  condebug: 'present' | 'absent' | 'unknown';
  launchOptions: string | null;
  manual: boolean;
}
export interface ChatConfig {
  manualPath: string | null;
  ownName: string | null;
  ignoreOwn: boolean;
}
export type ChatState = 'idle' | 'waiting' | 'following' | 'notfound';
export interface ChatStatus {
  state: ChatState;
  setup: Cs2Setup;
  config: ChatConfig;
  rulesVersion: string;
  rulesFile: string | null;
  rulesWarning: string | null;
  offset: number;
  messages: number;
  ownIgnored: number;
  duplicates: number;
  /** Lines that look like chat but match no rule. */
  unrecognized: number;
}

export const chat = {
  status: (): Promise<ChatStatus | null> => (isTauri ? invoke<ChatStatus>('chat_status') : Promise.resolve(null)),
  configure: (config: ChatConfig): Promise<ChatStatus | null> => (isTauri ? invoke<ChatStatus>('chat_configure', { config }) : Promise.resolve(null)),
  rescan: (): Promise<ChatStatus | null> => (isTauri ? invoke<ChatStatus>('chat_rescan') : Promise.resolve(null)),
  start: (): Promise<ChatStatus | null> => (isTauri ? invoke<ChatStatus>('chat_start') : Promise.resolve(null)),
  stop: (): Promise<void> => (isTauri ? invoke<void>('chat_stop') : Promise.resolve()),
  onStatus: (cb: (s: ChatStatus) => void) => (isTauri ? listen<ChatStatus>('chat://status', (e) => cb(e.payload)) : Promise.resolve(noop)),
  onMessage: (cb: (m: ChatMessage) => void) => (isTauri ? listen<ChatMessage>('chat://message', (e) => cb(e.payload)) : Promise.resolve(noop)),
};

// ---------- settings on disk (src-tauri/src/settings.rs) ----------

export interface StoredSettings {
  config: unknown;
  glossary: unknown;
  warnings: string[];
  dir: string;
}

export const settingsStore = {
  load: (): Promise<StoredSettings | null> => (isTauri ? invoke<StoredSettings>('settings_load') : Promise.resolve(null)),
  save: (config: unknown, glossary: unknown): Promise<void> => (isTauri ? invoke<void>('settings_save', { config, glossary }) : Promise.resolve()),
};

// ---------- in-app update (src-tauri/src/updater.rs) ----------

export type UpdateState = 'idle' | 'checking' | 'uptodate' | 'available' | 'downloading' | 'ready' | 'installing' | 'error';
export interface UpdateManifest {
  build: number;
  version: string;
  commit: string;
  date: string;
  notes: string[];
  setup: { url: string; sha256: string; bytes: number };
}
export interface UpdateStatus {
  state: UpdateState;
  build: number;
  version: string;
  commit: string;
  latest: UpdateManifest | null;
  doneBytes: number;
  totalBytes: number;
  error: string | null;
}

export const updater = {
  status: (): Promise<UpdateStatus | null> => (isTauri ? invoke<UpdateStatus>('update_status') : Promise.resolve(null)),
  check: (): Promise<UpdateStatus | null> => (isTauri ? invoke<UpdateStatus>('update_check') : Promise.resolve(null)),
  download: (): Promise<boolean> => (isTauri ? invoke<boolean>('update_download') : Promise.resolve(false)),
  cancel: (): Promise<void> => (isTauri ? invoke<void>('update_cancel') : Promise.resolve()),
  install: (): Promise<void> => (isTauri ? invoke<void>('update_install') : Promise.resolve()),
  onStatus: (cb: (s: UpdateStatus) => void) => (isTauri ? listen<UpdateStatus>('update://status', (e) => cb(e.payload)) : Promise.resolve(noop)),
};

// ---------- local history (src-tauri/src/history.rs) ----------

export interface HistoryEntry {
  ts: number;
  kind: 'voice' | 'chat';
  channel?: string;
  sender?: string;
  lang: string;
  original: string;
  translated?: string;
  status: string;
}

export const historyStore = {
  append: (entry: HistoryEntry): Promise<void> => (isTauri ? invoke<void>('history_append', { entry }) : Promise.resolve()),
  list: (query: string, limit: number): Promise<HistoryEntry[]> => (isTauri ? invoke<HistoryEntry[]>('history_list', { query, limit }) : Promise.resolve([])),
  count: (): Promise<number> => (isTauri ? invoke<number>('history_count') : Promise.resolve(0)),
  clear: (): Promise<void> => (isTauri ? invoke<void>('history_clear') : Promise.resolve()),
  exportTo: (format: 'txt' | 'json'): Promise<string> => (isTauri ? invoke<string>('history_export', { format }) : Promise.reject(new Error('export disponible dans l’application Windows'))),
};

// ---------- diagnostic journal and report (src-tauri/src/logs.rs) ----------

export interface LogLine {
  ts: number;
  level: 'INFO' | 'WARN' | 'ERROR';
  source: string;
  message: string;
}
export interface DiagReportRow {
  name: string;
  detail: string;
  state: string;
}

export const diagnostics = {
  recent: (limit: number): Promise<LogLine[]> => (isTauri ? invoke<LogLine[]>('logs_recent', { limit }) : Promise.resolve([])),
  write: (level: LogLine['level'], source: string, message: string): Promise<void> => (isTauri ? invoke<void>('logs_write', { level, source, message }) : Promise.resolve()),
  openFolder: (): Promise<string> => invoke<string>('logs_open_folder'),
  report: (rows: DiagReportRow[]): Promise<string> => invoke<string>('diag_report', { rows }),
  reportIssue: (exported: boolean): Promise<void> => invoke<void>('diag_report_issue', { exported }),
};

// ---------- map being played (src-tauri/src/gsi) ----------

export interface GsiStatus {
  installed: boolean;
  cfgPath: string | null;
  cs2Found: boolean;
  listening: boolean;
  port: number;
  error: string | null;
  map: string | null;
  mapName: string | null;
  mode: string | null;
  callouts: number;
  lastSeen: number | null;
}

export const gsi = {
  status: (): Promise<GsiStatus | null> => (isTauri ? invoke<GsiStatus>('gsi_status') : Promise.resolve(null)),
  install: (): Promise<GsiStatus> => invoke<GsiStatus>('gsi_install'),
  uninstall: (): Promise<GsiStatus> => invoke<GsiStatus>('gsi_uninstall'),
  onStatus: (cb: (s: GsiStatus) => void) => (isTauri ? listen<GsiStatus>('gsi://status', (e) => cb(e.payload)) : Promise.resolve(noop)),
};

// ---------- reply in the teammates' language (src-tauri/src/reply) ----------

export type ReplyTarget = 'en' | 'ru';
export interface ReplyPhrase {
  id: string;
  fr: string;
  quick: boolean;
}
export interface ReplyResult {
  text: string;
  target: ReplyTarget;
  via: 'phrasebook' | 'model';
  ms: number;
}
export interface ReplyModel {
  target: ReplyTarget;
  repo: string;
  sizeMb: number;
  installed: boolean;
  loaded: boolean;
}
export const REPLY_OPEN_EVENT = 'reply://open';

export const reply = {
  phrases: (): Promise<ReplyPhrase[]> => (isTauri ? invoke<ReplyPhrase[]>('reply_phrases') : Promise.resolve([])),
  phraseText: (id: string, target: ReplyTarget): Promise<string | null> => invoke<string | null>('reply_phrase_text', { id, target }),
  translate: (text: string, target: ReplyTarget): Promise<ReplyResult> => invoke<ReplyResult>('reply_translate', { text, target }),
  models: (): Promise<ReplyModel[]> => (isTauri ? invoke<ReplyModel[]>('reply_models') : Promise.resolve([])),
  open: (recentLangs: string[]): Promise<void> => (isTauri ? invoke<void>('reply_open', { recentLangs }) : Promise.resolve()),
  hide: (): Promise<void> => (isTauri ? invoke<void>('reply_hide') : Promise.resolve()),
  onOpen: (cb: (target: ReplyTarget) => void) => (isTauri ? listen<ReplyTarget>(REPLY_OPEN_EVENT, (e) => cb(e.payload)) : Promise.resolve(noop)),
};

// ---------- mode streamer: subtitles for OBS (src-tauri/src/stream) ----------

export interface StreamLine {
  kind: 'voice' | 'chat';
  lang: string;
  channel?: string;
  sender?: string;
  original?: string;
  translated?: string;
  test?: boolean;
}
export interface StreamStatus {
  options: import('../state/store').StreamOpts;
  listening: boolean;
  url: string;
  clients: number;
  sent: number;
  error: string | null;
}

export const streamOverlay = {
  status: (): Promise<StreamStatus | null> => (isTauri ? invoke<StreamStatus>('stream_status') : Promise.resolve(null)),
  configure: (options: import('../state/store').StreamOpts): Promise<StreamStatus | null> => (isTauri ? invoke<StreamStatus>('stream_configure', { options }) : Promise.resolve(null)),
  push: (line: StreamLine): Promise<boolean> => (isTauri ? invoke<boolean>('stream_push', { line }) : Promise.resolve(false)),
};
