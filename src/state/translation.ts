// Local French translation (Opus-MT on the CPU, no API key), mirrored from the
// Rust core (`translation://*` events).

import { langName, t } from '../i18n';
import { create } from 'zustand';
import { isTauri, translation, type PairStatus, type Translation, type TranslationConfig, type TranslationStatus } from '../backend';
import { GLOSSARY, type GlossEntry } from './data';
import type { AppState } from './store';
import { chosenSources } from './langs';

export interface TranslationState {
  native: boolean;
  status: TranslationStatus | null;
  /** Recent translation times (ms, cache hits excluded), newest last. */
  times: number[];
}

export const useTranslation = create<TranslationState>()(() => ({ native: isTauri, status: null, times: [] }));

/** Rust translation settings from the UI: on/off switch and glossary. */
export function translationConfigFrom(s: Pick<AppState, 'tg' | 'gloss'> & Partial<Pick<AppState, 'target' | 'src'>>): TranslationConfig {
  const target = s.target ?? 'fr';
  // The built-in renderings are French: into another language the term is kept as spoken.
  const builtIn = (term: string, rendering: string) => GLOSSARY.some(([t, m, r]) => t === term && m === 'Traduire' && r === rendering);
  return {
    enabled: s.tg.translate,
    glossary: s.gloss.map(([source, mode, rendering]: GlossEntry) => ({ source, target: mode === 'Traduire' && (target === 'fr' || !builtIn(source, rendering)) ? rendering : null })),
    target,
    sources: s.src ? chosenSources(s.src) : ['ru', 'en'],
  };
}

const cap = (s: string) => s.charAt(0) + s.slice(1).toLowerCase();

/** "Russe → français" in the interface's language. */
export function pairName(source: string, target: string): string {
  return `${cap(langName(source))} → ${langName(target).toLowerCase()}`;
}

export interface PairSummary {
  label: string;
  value: string;
  color: string;
}

export function pairSummary(p: PairStatus): PairSummary {
  const label = pairName(p.source, p.target ?? 'fr');
  switch (p.state) {
    case 'ready':
      return { label, value: p.loadMs != null ? `prêt · chargé en ${p.loadMs} ms` : 'prêt', color: '#7fe0a8' };
    case 'loading':
      return { label, value: 'chargement…', color: '#ffa24a' };
    case 'error':
      return { label, value: p.error ?? 'erreur', color: '#ff8a7a' };
    case 'missing':
      return { label, value: 'modèle absent', color: '#ffa24a' };
  }
}

/** One line for the pipeline, the status cards and the diagnostics. */
export function translationSummary(st: TranslationStatus | null): PairSummary {
  const label = t('tr.label');
  if (!st) return { label, value: '…', color: '#8f8a83' };
  if (!st.config.enabled) return { label, value: t('tr.disabled'), color: '#8f8a83' };
  const ready = st.pairs.filter((p) => p.state === 'ready').map((p) => p.source.toUpperCase());
  if (st.pairs.some((p) => p.state === 'loading')) return { label, value: t('tr.loading'), color: '#ffa24a' };
  if (st.pairs.some((p) => p.state === 'error')) return { label, value: t('tr.error'), color: '#ff8a7a' };
  if (!ready.length) return { label, value: t('tr.missing'), color: '#ffa24a' };
  return { label, value: t('tr.ready', { langs: ready.join(', '), target: (st.config.target ?? 'fr').toUpperCase() }), color: '#7fe0a8' };
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

export async function rescanTranslation(): Promise<void> {
  const status = await translation.rescan();
  if (status) useTranslation.setState({ status });
}
