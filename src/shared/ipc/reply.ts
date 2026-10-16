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
  /** Exact hand-written CS phrase, or the connected LLM. */
  via: 'phrasebook' | 'llm';
  ms: number;
}
/** A language the reply can be written in, and whether free text works now. */
export interface ReplyLanguage {
  /** The user's language. */
  source: string;
  target: ReplyTarget;
  /** The LLM is connected: any sentence can be translated. */
  ready: boolean;
  /** Why free text is unavailable (the CS phrases still work). */
  problem: string | null;
}
export const REPLY_OPEN_EVENT = 'reply://open';

export const reply = {
  phrases: (): Promise<ReplyPhrase[]> => (isTauri ? invoke<ReplyPhrase[]>('reply_phrases') : Promise.resolve([])),
  phraseText: (id: string, target: ReplyTarget): Promise<string | null> => invoke<string | null>('reply_phrase_text', { id, target }),
  translate: (text: string, target: ReplyTarget): Promise<ReplyResult> => invoke<ReplyResult>('reply_translate', { text, target }),
  models: (): Promise<ReplyLanguage[]> => (isTauri ? invoke<ReplyLanguage[]>('reply_models') : Promise.resolve([])),
  open: (recentLangs: string[]): Promise<void> => (isTauri ? invoke<void>('reply_open', { recentLangs }) : Promise.resolve()),
  hide: (): Promise<void> => (isTauri ? invoke<void>('reply_hide') : Promise.resolve()),
  /** Closes the pop-up (CS2 back in front), then the AI voice says the text in the game. Returns its length in ms. */
  say: (text: string, target: ReplyTarget): Promise<number> => invoke<number>('reply_say', { text, target }),
  onOpen: (cb: (target: ReplyTarget) => void) => (isTauri ? listen<ReplyTarget>(REPLY_OPEN_EVENT, (e) => cb(e.payload)) : Promise.resolve(noop)),
};
