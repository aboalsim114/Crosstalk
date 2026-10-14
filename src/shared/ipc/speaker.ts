// "Qui parle ?": names read on CS2's voice indicator (src-tauri/src/speaker).

import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { isTauri, noop } from './core';

export interface SpeakerRegion {
  x: number;
  y: number;
  w: number;
  h: number;
}
export interface SpeakerConfig {
  enabled: boolean;
  region: SpeakerRegion;
  ignore: string[];
}
export interface SpeakerStatus {
  enabled: boolean;
  state: 'off' | 'waiting' | 'paused' | 'watching' | 'error';
  detail: string | null;
  ocrLang: string | null;
  known: string[];
  blank: boolean;
  covered: boolean;
}
export interface SpeakerTest {
  preview: string | null;
  lines: string[];
  names: string[];
  blank: boolean;
  covered: boolean;
  foreground: boolean;
  error: string | null;
}
export const SPEAKER_STATUS_EVENT = 'speaker://status';

export const speakerId = {
  status: (): Promise<SpeakerStatus | null> => (isTauri ? invoke<SpeakerStatus>('speaker_status') : Promise.resolve(null)),
  configure: (config: SpeakerConfig): Promise<SpeakerStatus | null> => (isTauri ? invoke<SpeakerStatus>('speaker_configure', { config }) : Promise.resolve(null)),
  test: (region: SpeakerRegion, delayMs: number): Promise<SpeakerTest | null> => (isTauri ? invoke<SpeakerTest>('speaker_test', { region, delayMs }) : Promise.resolve(null)),
  onStatus: (cb: (s: SpeakerStatus) => void) => (isTauri ? listen<SpeakerStatus>(SPEAKER_STATUS_EVENT, (e) => cb(e.payload)) : Promise.resolve(noop)),
};
