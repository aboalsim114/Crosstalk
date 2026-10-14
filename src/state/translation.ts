// Local French translation (TranslateGemma 4B, Opus-MT as fallback, no API
// key), mirrored from the Rust core (`translation://*` events).

import { t } from '../i18n';
import { create } from 'zustand';
import { isTauri, translation, type DownloadStatus, type LlmStatus, type PairStatus, type Translation, type TranslationConfig, type TranslationStatus } from '../backend';
import type { GlossEntry } from './data';
import type { AppState } from './store';

export interface TranslationState {
  native: boolean;
  status: TranslationStatus | null;
  /** Recent translation times (ms, cache hits excluded), newest last. */
  times: number[];
}

export const useTranslation = create<TranslationState>()(() => ({ native: isTauri, status: null, times: [] }));

/** Rust translation settings from the UI: on/off switch and glossary. */
export function translationConfigFrom(s: Pick<AppState, 'tg' | 'gloss'> & Partial<Pick<AppState, 'trEngine' | 'llmBackend'>>): TranslationConfig {
  return {
    enabled: s.tg.translate,
    glossary: s.gloss.map(([source, mode, rendering]: GlossEntry) => ({ source, target: mode === 'Traduire' ? rendering : null })),
    engine: s.trEngine ?? 'auto',
    llmBackend: s.llmBackend ?? 'auto',
    llmThreads: 0,
  };
}

export interface LlmView {
  /** Short state shown in the card's chip. */
  state: string;
  color: string;
  /** One sentence under it (what happens, what to do). */
  detail: string;
  canDownload: boolean;
  canVerify: boolean;
  /** Download of the model in progress: percent, MB, speed. */
  progress: { pct: number; text: string } | null;
  backend: string | null;
  timing: string | null;
}

const gb = (mb: number) => `${(mb / 1000).toFixed(2).replace('.', ',')} Go`;

/** TranslateGemma's card from its status and the model download. */
export function llmView(st: LlmStatus | null | undefined, dl: DownloadStatus | null): LlmView {
  const base: LlmView = { state: '…', color: '#8f8a83', detail: '', canDownload: false, canVerify: false, progress: null, backend: null, timing: null };
  if (!st) return { ...base, detail: 'Disponible dans l’application Windows.' };
  const downloading = dl?.state === 'downloading' && (dl.current ?? '').includes(st.name);
  if (downloading && dl) {
    const pct = dl.totalBytes > 0 ? Math.min(100, Math.floor((dl.doneBytes / dl.totalBytes) * 100)) : 0;
    const speed = dl.bytesPerSec ? ` · ${(dl.bytesPerSec / 1_000_000).toFixed(1).replace('.', ',')} Mo/s` : '';
    return {
      ...base,
      state: dl.verifying ? 'VÉRIFICATION' : 'TÉLÉCHARGEMENT',
      color: '#ffa24a',
      detail: dl.verifying ? 'Contrôle de l’empreinte SHA-256 avant la première utilisation.' : 'Le téléchargement reprend où il s’est arrêté en cas de coupure.',
      progress: { pct, text: `${Math.round(dl.doneBytes / 1_000_000)} / ${Math.round(dl.totalBytes / 1_000_000)} Mo · ${pct} %${speed}` },
    };
  }
  const backend = st.backend ? (st.backend === 'cpu' ? `Processeur · ${st.threads ?? '?'} threads` : `Carte graphique (${st.backend})${st.device ? ` · ${st.device}` : ''}`) : null;
  const timing = st.p50Ms != null ? `p50 ${st.p50Ms} ms · p95 ${st.p95Ms ?? st.p50Ms} ms · ${st.translations} phrases` : null;
  const ready = { ...base, backend, timing, canVerify: true };
  switch (st.state) {
    case 'missing':
      if (!st.helper) return { ...base, state: 'MOTEUR ABSENT', color: '#ff8a7a', detail: 'crosstalk-llm.exe manque : réinstallez Crosstalk. Opus-MT est utilisé en attendant.' };
      return {
        ...base,
        state: st.install.state === 'partial' ? 'INCOMPLET' : st.install.state === 'corrupt' ? 'CORROMPU' : 'NON INSTALLÉ',
        color: '#ffa24a',
        detail: `${gb(st.sizeMb)} à télécharger une seule fois, puis tout fonctionne hors ligne. Opus-MT est utilisé en attendant.`,
        canDownload: true,
      };
    case 'unverified':
      return { ...base, state: 'À VÉRIFIER', color: '#ffa24a', detail: 'Fichier présent mais jamais contrôlé : vérifiez-le avant de l’utiliser.', canVerify: true, canDownload: true };
    case 'off':
      return { ...ready, state: 'NON UTILISÉ', color: '#8f8a83', detail: st.choice === 'opus' ? 'Opus-MT est choisi : TranslateGemma n’est pas chargé.' : 'Traduction désactivée : rien n’est chargé.' };
    case 'idle':
      return { ...ready, state: 'INSTALLÉ', color: '#7fe0a8', detail: 'Chargé au démarrage de la traduction (une fois par session).' };
    case 'loading':
      return { ...ready, state: 'CHARGEMENT', color: '#ffa24a', detail: 'Chargement du modèle en mémoire…' };
    case 'ready':
      return { ...ready, state: 'PRÊT', color: '#7fe0a8', detail: `Chargé en ${st.loadMs != null ? `${(st.loadMs / 1000).toFixed(1).replace('.', ',')} s` : '—'}${st.rssMb ? ` · ${st.rssMb} Mo de mémoire` : ''}${st.gpuSkipped ? ` · GPU non utilisé : ${st.gpuSkipped}` : ''}.` };
    case 'slow':
      return { ...ready, state: 'TROP LENT', color: '#ffa24a', detail: 'Trop lent sur ce PC pour des annonces en direct : Opus-MT prend le relais. Choisissez « TranslateGemma » pour l’imposer.' };
    case 'error':
      return { ...ready, state: 'ERREUR', color: '#ff8a7a', detail: `${st.error ?? 'Erreur inconnue'} · Opus-MT prend le relais.`, canDownload: st.install.state !== 'verified' };
  }
}

export const PAIR_NAME: Record<string, string> = { ru: 'Russe → français', en: 'Anglais → français' };

export interface PairSummary {
  label: string;
  value: string;
  color: string;
}

export function pairSummary(p: PairStatus): PairSummary {
  const label = PAIR_NAME[p.source] ?? p.source;
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
  return { label, value: t('tr.ready', { langs: ready.join(', ') }), color: '#7fe0a8' };
}

export function percentile(times: number[], q: number): number | null {
  if (!times.length) return null;
  const sorted = [...times].sort((a, b) => a - b);
  return sorted[Math.min(sorted.length - 1, Math.floor(q * sorted.length))];
}

/** Keeps the last 50 real translation times (cache hits excluded). */
export function recordTime(t: Translation): void {
  if (!t.translated || t.cached || t.lang === 'fr') return;
  useTranslation.setState((s) => ({ times: [...s.times, t.ms].slice(-50) }));
}

export async function rescanTranslation(): Promise<void> {
  const status = await translation.rescan();
  if (status) useTranslation.setState({ status });
}
