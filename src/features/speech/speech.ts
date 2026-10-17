// Real speech recognition, mirrored from the Rust core (`speech://*` events).
// A transcript is shown in the speaker's language until its real French
// translation arrives (`translation://result`, see state/translation.ts).

import { t } from '../../shared/i18n';
import { create } from 'zustand';
import { isTauri, speech, type ModelInfo, type RejectReason, type SpeechConfig, type SpeechStats, type SpeechStatus, type Transcript, type VoiceActivity } from '../../shared/ipc';
import type { AppState, FeedEntry } from '../../app/state/store';
import { SPEECH_LANGS } from '../languages/langs';

export interface SpeechState {
  native: boolean;
  status: SpeechStatus | null;
  stats: SpeechStats | null;
  models: ModelInfo[];
  activity: VoiceActivity;
  /** Core → interface delay of the last final lines (ms). */
  display: number[];
}

export const useSpeech = create<SpeechState>()(() => ({
  native: isTauri,
  status: null,
  stats: null,
  models: [],
  activity: { speaking: false, transcribing: false },
  display: [],
}));

/** Core → interface delay of a final transcript (ms), when the core stamped it. */
export function displayDelay(t: Transcript, now = Date.now()): number | null {
  return t.partial || !t.emittedAtMs ? null : Math.max(0, now - t.emittedAtMs);
}

/** Keeps the last 100 display delays. */
export function noteDisplay(delays: number[], d: number | null): number[] {
  return d == null ? delays : [...delays, d].slice(-100);
}

/** "fin de phrase 610/640 ms · attente 0/350 · calcul 280/420 · affichage 3/9" (p50/p95). */
export function stagesLine(st: SpeechStats | null, display: number[]): string | null {
  const s = st?.stages;
  if (!s || s.close.p50Ms == null) return null;
  const pair = (p: { p50Ms: number | null; p95Ms: number | null }) => (p.p50Ms == null ? '—' : `${p.p50Ms}/${p.p95Ms ?? p.p50Ms}`);
  const sorted = [...display].sort((a, b) => a - b);
  const pick = (q: number) => sorted[Math.min(sorted.length - 1, Math.floor(q * sorted.length))];
  const shown = sorted.length ? ` · affichage ${pick(0.5)}/${pick(0.95)}` : '';
  return `fin de phrase ${pair(s.close)} ms · attente ${pair(s.queue)} · calcul ${pair(s.asr)}${shown}`;
}

// ---------- pure helpers (unit-tested) ----------

/** Rust speech settings from the UI settings (threshold %, languages, model). */
export function speechConfigFrom(s: Pick<AppState, 'model' | 'vad'> & Partial<Pick<AppState, 'voiceFocus'>>): SpeechConfig {
  // Every language the recogniser writes: the spoken language is detected, never chosen.
  return {
    model: s.model,
    languages: [...SPEECH_LANGS],
    vadThreshold: Math.min(0.9, Math.max(0.2, s.vad / 100)),
    voiceFocus: s.voiceFocus ?? false,
  };
}

/** A real transcript as a Script / overlay line: original text, translation pending. */
export function transcriptEntry(t: Transcript, now = new Date()): FeedEntry {
  return {
    kind: 'voice',
    src: t.lang,
    orig: t.text,
    tr: null,
    id: t.id + Math.random() / 10,
    tid: t.id,
    at: now.getTime(),
    time: now.toLocaleTimeString('fr-FR'),
    status: 'transcrit',
    shown: null,
    lat: t.partial ? null : t.latencyMs,
    err: false,
    live: t.partial,
    ...(t.speaker ? { sender: t.speaker } : {}),
  };
}

export interface EngineSummary {
  value: string;
  detail: string;
  color: string;
}

export function engineSummary(st: SpeechStatus | null): EngineSummary {
  if (!st) return { value: '…', detail: t('eng.starting'), color: '#8f8a83' };
  const model = st.modelLabel || st.config.model;
  switch (st.state) {
    case 'ready':
      return {
        value: model,
        detail: st.loadMs != null ? t('eng.loaded', { backend: st.backend, threads: st.threads, ms: formatMs(st.loadMs) }) : t('eng.threads', { backend: st.backend, threads: st.threads }),
        color: '#7fe0a8',
      };
    case 'loading':
      return { value: t('eng.loading'), detail: `${model} · ${st.backend}`, color: '#ffa24a' };
    case 'error':
      return { value: t('eng.error'), detail: st.error ?? t('eng.unavailable'), color: '#ff8a7a' };
    case 'missing':
      return {
        value: !st.vadReady ? t('eng.noVad') : t('eng.noModel'),
        detail: !st.vadReady ? t('eng.noVadDetail') : t('eng.notInstalled', { model }),
        color: '#ffa24a',
      };
  }
}

export function formatMs(ms: number): string {
  return ms < 1000 ? `${Math.round(ms)} ms` : `${(ms / 1000).toFixed(1).replace('.', ',')} s`;
}

export const REJECT_LABEL: Record<RejectReason, string> = {
  short: 'trop court',
  transient: 'tir / explosion',
  stale: 'périmé',
  overflow: 'file pleine',
  empty: 'vide',
  hallucination: 'hallucination',
  repetition: 'répétition',
  wrongScript: 'alphabet incohérent',
};

/** One-line summary of the counters, e.g. "12 transcrits · 5 rejetés (tir / explosion 3, …)". */
export function statsLine(st: SpeechStats | null): string {
  if (!st || st.segments === 0) return 'Aucun segment vocal pour l’instant';
  const rej = Object.entries(st.rejected) as [RejectReason, number][];
  const total = rej.reduce((n, [, v]) => n + v, 0);
  const detail = rej
    .sort((a, b) => b[1] - a[1])
    .map(([r, n]) => `${REJECT_LABEL[r]} ${n}`)
    .join(', ');
  return `${st.transcribed} transcrits · ${total} rejetés${detail ? ` (${detail})` : ''}`;
}

// ---------- actions ----------

export async function refreshModels(): Promise<void> {
  const models = await speech.models().catch(() => []);
  useSpeech.setState({ models });
}

export async function rescanModels(): Promise<void> {
  const status = await speech.rescan().catch(() => null);
  if (status) useSpeech.setState({ status });
  await refreshModels();
}
