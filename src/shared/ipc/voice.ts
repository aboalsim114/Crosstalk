// "Voix IA" (src-tauri/src/voice): dictated replies said in the game by a
// local Piper voice, through VB-CABLE, CS2's voice key held automatically
// when "PTT automatique" is on (src-tauri/src/voice/ptt.rs).

import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { isTauri, noop } from './core';
import type { DownloadStatus } from './downloads';

export interface VoiceOptions {
  enabled: boolean;
  /** Real microphone id (null: Windows' default). */
  mic: string | null;
  /** Chosen voice per language. */
  voices: Record<string, string>;
  monitor: boolean;
  volume: number;
  micVolume: number;
  /** Hold CS2's voice key while the AI voice plays. */
  autoPtt: boolean;
  /** CS2's voice bind ("K", "V", "Mouse4"…). */
  pttKey: string;
  /** Key held before the first sample / after the last one (ms). */
  pttLeadMs: number;
  pttTailMs: number;
  /** Also say the replies typed in the pop-up (Mouse 4) when Enter is pressed. */
  speakTyped?: boolean;
}

/** Where an automatic push-to-talk transmission is. */
export type PttPhase = 'idle' | 'preparing' | 'synthesized' | 'held' | 'playing' | 'releasing' | 'done' | 'cancelled' | 'error';

export interface PttStatus {
  phase: PttPhase;
  /** Why the key was not pressed, or what went wrong. */
  detail: string | null;
  /** Crosstalk pressed the key for this sentence. */
  pressed: boolean;
}

export interface VoiceInfo {
  id: string;
  lang: string;
  name: string;
  gender: 'female' | 'male';
  onnxBytes: number;
  jsonBytes: number;
  installed: boolean;
}

export interface VoiceDevice {
  id: string;
  name: string;
  isDefault: boolean;
}

export interface VoiceStatus {
  options: VoiceOptions;
  engineReady: boolean;
  voices: VoiceInfo[];
  cable: VoiceDevice | null;
  mics: VoiceDevice[];
  routing: boolean;
  speaking: boolean;
  error: string | null;
  download: DownloadStatus;
  ptt: PttStatus;
}

export interface SpeakState {
  text: string;
  lang: string;
  ms: number;
}

/** Size of the Piper engine archive (src-tauri/src/voice/catalog.rs). */
export const VOICE_ENGINE_BYTES = 22_477_236;

export const voice = {
  status: (): Promise<VoiceStatus | null> => (isTauri ? invoke<VoiceStatus>('voice_status') : Promise.resolve(null)),
  configure: (options: VoiceOptions): Promise<VoiceStatus | null> => (isTauri ? invoke<VoiceStatus>('voice_configure', { options }) : Promise.resolve(null)),
  download: (voices: string[]): Promise<DownloadStatus | null> => (isTauri ? invoke<DownloadStatus>('voice_download', { voices }) : Promise.resolve(null)),
  cancelDownload: (): Promise<void> => (isTauri ? invoke<void>('voice_download_cancel') : Promise.resolve()),
  /** Says `text` in the game (cable). Resolves with its length in ms. */
  say: (text: string, lang: string): Promise<number> => (isTauri ? invoke<number>('voice_say', { text, lang }) : Promise.reject(new Error('application Windows requise'))),
  /** Plays a voice's test sentence in the headset. */
  preview: (voiceId: string): Promise<number> => (isTauri ? invoke<number>('voice_preview', { voice: voiceId }) : Promise.reject(new Error('application Windows requise'))),
  /** Stops the AI voice now; the voice key is released at once. */
  cancel: (): Promise<void> => (isTauri ? invoke<void>('voice_cancel') : Promise.resolve()),
  /** Waits `delayS` (switch to CS2), then says the test sentence of `lang` with the current settings. */
  pttTest: (lang: string, delayS: number): Promise<number> => (isTauri ? invoke<number>('voice_ptt_test', { lang, delayS }) : Promise.reject(new Error('application Windows requise'))),
  onPtt: (cb: (s: PttStatus) => void) => (isTauri ? listen<PttStatus>('voice://ptt', (e) => cb(e.payload)) : Promise.resolve(noop)),
  openCablePage: (): Promise<void> => (isTauri ? invoke<void>('voice_open_cable_page') : Promise.resolve()),
  onDownload: (cb: (s: DownloadStatus) => void) => (isTauri ? listen<DownloadStatus>('voice://download', (e) => cb(e.payload)) : Promise.resolve(noop)),
  onSpeak: (cb: (s: SpeakState) => void) => (isTauri ? listen<SpeakState>('voice://state', (e) => cb(e.payload)) : Promise.resolve(noop)),
};
