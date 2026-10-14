// Spoken reply (src-tauri/src/dictation.rs).

import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { isTauri, noop } from './core';
import type { ReplyTarget } from './reply';

export interface DictationState {
  state: 'listening' | 'transcribing' | 'done' | 'error';
  level: number;
  seconds: number;
  text: string | null;
  translated: string | null;
  target: ReplyTarget;
  via: 'phrasebook' | 'model' | null;
  copied: boolean;
  error: string | null;
}
export const DICTATION_EVENT = 'dictation://state';

export const dictation = {
  /** Starts a dictation (opens the reply window), or finishes the running one. */
  toggle: (recentLangs: string[], target?: ReplyTarget): Promise<void> => (isTauri ? invoke<void>('dictation_toggle', { recentLangs, target: target ?? null }) : Promise.resolve()),
  state: (): Promise<DictationState | null> => (isTauri ? invoke<DictationState | null>('dictation_state') : Promise.resolve(null)),
  onState: (cb: (s: DictationState) => void) => (isTauri ? listen<DictationState>(DICTATION_EVENT, (e) => cb(e.payload)) : Promise.resolve(noop)),
};
