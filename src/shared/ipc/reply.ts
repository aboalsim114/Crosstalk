// Reply in the teammates' language (src-tauri/src/reply).

import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { isTauri, noop } from './core';

/** A players' language Crosstalk can write into. */
export type ReplyTarget = string;
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
  /** The user's language. */
  source: string;
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
