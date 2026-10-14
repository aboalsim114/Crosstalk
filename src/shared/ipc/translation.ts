// Local translation (Opus-MT, no API key).

import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { isTauri, noop } from './core';

export interface TranslationTerm {
  source: string;
  /** Imposed French rendering; null keeps the term as spoken. */
  target: string | null;
}
export interface TranslationConfig {
  enabled: boolean;
  glossary: TranslationTerm[];
  /** The user's language: everything is translated into it. */
  target?: string;
  /** The players' languages. */
  sources?: string[];
}
export type PairState = 'missing' | 'loading' | 'ready' | 'error';
export interface PairStatus {
  source: string;
  target?: string;
  /** "A + B" when the route goes through English. */
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
  /** Language of `translated` (the user's language when it was sent). */
  target?: string;
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
