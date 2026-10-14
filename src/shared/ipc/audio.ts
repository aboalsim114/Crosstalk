// Audio capture (src-tauri/src/audio) and CS2 detection (src-tauri/src/game).

import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { isTauri, noop } from './core';

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
