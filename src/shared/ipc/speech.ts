// Speech recognition (src-tauri/src/speech).

import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { isTauri, noop } from './core';

/** Recognised languages: the players' and the user's own (src/features/languages/langs.ts). */
export type SpeechLang = 'fr' | 'en' | 'ru' | 'de' | 'es' | 'uk' | 'pl' | 'tr';
export type EngineState = 'missing' | 'loading' | 'ready' | 'error';
export type RejectReason =
  | 'short'
  | 'transient'
  | 'stale'
  | 'overflow'
  | 'empty'
  | 'hallucination'
  | 'repetition'
  | 'wrongScript';

export interface SpeechConfig {
  model: string;
  languages: SpeechLang[];
  /** 0.2–0.9. */
  vadThreshold: number;
  /** "Focus voix": keep the centre of the stereo game audio (voice chat), turn down side sounds. */
  voiceFocus?: boolean;
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
  /** Name of the recognition model, e.g. "Parakeet TDT 0.6B v3". */
  modelLabel: string;
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
  model: string;
  /** Name read on CS2's voice indicator while it was spoken ("Qui parle ?", opt-in). */
  speaker?: string;
}

export interface SpeechStats {
  segments: number;
  transcribed: number;
  rejected: Partial<Record<RejectReason, number>>;
  droppedBlocks: number;
  latencyP50Ms: number | null;
  latencyP95Ms: number | null;
  lastRtf: number | null;
  /** Recogniser time and audio duration of every final segment (cumulative, ms). */
  asrMs?: number;
  asrAudioMs?: number;
  cpuPercent: number | null;
  /** Game sound removed by "Focus voix" over the last second (dB); null when off or mono. */
  focusDb?: number | null;
}

export interface VoiceActivity {
  speaking: boolean;
  transcribing: boolean;
}

export interface ModelInfo {
  id: string;
  /** Display name, e.g. "Parakeet TDT 0.6B v3". */
  label: string;
  sizeMb: number;
  note: string;
  /** Licence of the model weights. */
  license: string;
  installed: boolean;
}

/** Whisper files left by earlier versions (deleted on request only). */
export interface LegacyWhisper {
  files: number;
  bytes: number;
}

export const speech = {
  status: (): Promise<SpeechStatus | null> => (isTauri ? invoke<SpeechStatus>('speech_status') : Promise.resolve(null)),
  stats: (): Promise<SpeechStats | null> => (isTauri ? invoke<SpeechStats>('speech_stats') : Promise.resolve(null)),
  models: (): Promise<ModelInfo[]> => (isTauri ? invoke<ModelInfo[]>('speech_models') : Promise.resolve([])),
  configure: (config: SpeechConfig): Promise<SpeechStatus | null> =>
    isTauri ? invoke<SpeechStatus>('speech_configure', { config }) : Promise.resolve(null),
  rescan: (): Promise<SpeechStatus | null> => (isTauri ? invoke<SpeechStatus>('speech_rescan') : Promise.resolve(null)),
  legacyWhisper: (): Promise<LegacyWhisper | null> => (isTauri ? invoke<LegacyWhisper>('speech_legacy_whisper') : Promise.resolve(null)),
  /** Removes the old Whisper files: only on the user's request. */
  legacyCleanup: (): Promise<LegacyWhisper | null> => (isTauri ? invoke<LegacyWhisper>('speech_legacy_cleanup') : Promise.resolve(null)),
  onStatus: (cb: (s: SpeechStatus) => void) => (isTauri ? listen<SpeechStatus>('speech://status', (e) => cb(e.payload)) : Promise.resolve(noop)),
  onTranscript: (cb: (t: Transcript) => void) => (isTauri ? listen<Transcript>('speech://transcript', (e) => cb(e.payload)) : Promise.resolve(noop)),
  /** The live line of this utterance must go (its segment was finally rejected). */
  onRetract: (cb: (id: number) => void) => (isTauri ? listen<number>('speech://retract', (e) => cb(e.payload)) : Promise.resolve(noop)),
  onActivity: (cb: (a: VoiceActivity) => void) => (isTauri ? listen<VoiceActivity>('speech://activity', (e) => cb(e.payload)) : Promise.resolve(noop)),
  onStats: (cb: (s: SpeechStats) => void) => (isTauri ? listen<SpeechStats>('speech://stats', (e) => cb(e.payload)) : Promise.resolve(noop)),
};
