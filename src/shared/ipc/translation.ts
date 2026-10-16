// Translation into the user's language by the LLM the user connected
// (./llm.ts): voice lines, chat, replies. No local translation model.

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
export interface TranslationStatus {
  config: TranslationConfig;
  /** The LLM can be asked. */
  ready: boolean;
  /** Why translations are unavailable (shown as is). */
  problem: string | null;
  queued: number;
}
/** Opus-MT folders left by earlier versions. */
export interface LegacyModels {
  folders: number;
  bytes: number;
}
/** Translation of a line (matched by `id`), or why it is missing: the original then stays shown. */
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
  /** What produced `translated`: the LLM, or the built-in callouts. */
  engine?: 'llm' | 'lexicon' | null;
}

export const translation = {
  status: (): Promise<TranslationStatus | null> => (isTauri ? invoke<TranslationStatus>('translation_status') : Promise.resolve(null)),
  configure: (config: TranslationConfig): Promise<TranslationStatus | null> =>
    isTauri ? invoke<TranslationStatus>('translation_configure', { config }) : Promise.resolve(null),
  rescan: (): Promise<TranslationStatus | null> => (isTauri ? invoke<TranslationStatus>('translation_rescan') : Promise.resolve(null)),
  test: (lang: string, text: string): Promise<Translation | null> =>
    isTauri ? invoke<Translation>('translation_test', { lang, text }) : Promise.resolve(null),
  legacyModels: (): Promise<LegacyModels | null> => (isTauri ? invoke<LegacyModels>('translation_legacy_models') : Promise.resolve(null)),
  /** Deletes the old Opus-MT folders (on the user's request only). */
  legacyCleanup: (): Promise<LegacyModels | null> => (isTauri ? invoke<LegacyModels>('translation_legacy_cleanup') : Promise.resolve(null)),
  onStatus: (cb: (s: TranslationStatus) => void) =>
    isTauri ? listen<TranslationStatus>('translation://status', (e) => cb(e.payload)) : Promise.resolve(noop),
  onResult: (cb: (t: Translation) => void) => (isTauri ? listen<Translation>('translation://result', (e) => cb(e.payload)) : Promise.resolve(noop)),
};
