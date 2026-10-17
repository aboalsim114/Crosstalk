// Translation into the user's language by the connected LLM, mirrored from
// the Rust core (`translation://*` events).

import { t } from '../../shared/i18n';
import { create } from 'zustand';
import { isTauri, translation, type Translation, type TranslationConfig, type TranslationStatus } from '../../shared/ipc';
import { GLOSSARY, type GlossEntry } from '../../app/state/data';
import type { AppState } from '../../app/state/store';
import { PLAYER_LANGS } from '../languages/langs';

export interface TranslationState {
  native: boolean;
  status: TranslationStatus | null;
  /** Recent translation times (ms, cache hits excluded), newest last. */
  times: number[];
}

export const useTranslation = create<TranslationState>()(() => ({ native: isTauri, status: null, times: [] }));

/** Rust translation settings from the UI: on/off switch and glossary. */
export function translationConfigFrom(s: Pick<AppState, 'tg' | 'gloss'> & Partial<Pick<AppState, 'target'>>): TranslationConfig {
  const target = s.target ?? 'fr';
  // The built-in renderings are French: into another language the term is kept as spoken.
  const builtIn = (term: string, rendering: string) => GLOSSARY.some(([t, m, r]) => t === term && m === 'Traduire' && r === rendering);
  return {
    enabled: s.tg.translate,
    glossary: s.gloss.map(([source, mode, rendering]: GlossEntry) => ({ source, target: mode === 'Traduire' && (target === 'fr' || !builtIn(source, rendering)) ? rendering : null })),
    target,
    // Every detected language is translated: there is nothing to choose.
    sources: [...PLAYER_LANGS],
  };
}

export interface TranslationSummary {
  label: string;
  value: string;
  color: string;
}

/** State of the translation as a whole: what the colours and the diagnostics derive from. */
export type TranslationHealth = 'off' | 'missing' | 'ready';

export function translationHealth(st: TranslationStatus | null): TranslationHealth {
  if (!st || !st.config.enabled) return 'off';
  return st.ready ? 'ready' : 'missing';
}

const HEALTH_COLOR: Record<TranslationHealth, string> = { off: '#8f8a83', missing: '#ffa24a', ready: '#7fe0a8' };

/** One line for the pipeline, the status cards and the diagnostics. */
export function translationSummary(st: TranslationStatus | null): TranslationSummary {
  const label = t('tr.label');
  const health = translationHealth(st);
  const color = HEALTH_COLOR[health];
  if (!st) return { label, value: '…', color };
  if (health === 'off') return { label, value: t('tr.disabled'), color };
  if (health === 'missing') return { label, value: t('tr.missing'), color };
  const target = st.config.target ?? 'fr';
  const langs = (st.config.sources ?? []).filter((l) => l !== target).map((l) => l.toUpperCase());
  return { label, value: t('tr.ready', { langs: langs.join(', ') || '—', target: target.toUpperCase() }), color };
}

export function percentile(times: number[], q: number): number | null {
  if (!times.length) return null;
  const sorted = [...times].sort((a, b) => a - b);
  return sorted[Math.min(sorted.length - 1, Math.floor(q * sorted.length))];
}

/** Keeps the last 50 real translation times (cache hits excluded). */
export function recordTime(t: Translation): void {
  if (!t.translated || t.cached || t.lang === (t.target || 'fr')) return;
  useTranslation.setState((s) => ({ times: [...s.times, t.ms].slice(-50) }));
}

export async function refreshTranslation(): Promise<void> {
  const status = await translation.rescan();
  if (status) useTranslation.setState({ status });
}
