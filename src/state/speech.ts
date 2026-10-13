// Real speech recognition, mirrored from the Rust core (`speech://*` events).
// A transcript is shown in the speaker's language until its real French
// translation arrives (`translation://result`, see state/translation.ts).

import { create } from 'zustand';
import { isTauri, speech, type ModelInfo, type RejectReason, type SpeechConfig, type SpeechStats, type SpeechStatus, type Transcript, type VoiceActivity } from '../backend';
import type { GlossEntry } from './data';
import type { AppState, FeedEntry } from './store';

export interface SpeechState {
  native: boolean;
  status: SpeechStatus | null;
  stats: SpeechStats | null;
  models: ModelInfo[];
  activity: VoiceActivity;
}

export const useSpeech = create<SpeechState>()(() => ({
  native: isTauri,
  status: null,
  stats: null,
  models: [],
  activity: { speaking: false, transcribing: false },
}));

// ---------- pure helpers (unit-tested) ----------

/** Rust speech settings from the UI settings (threshold %, languages, glossary, model). */
export function speechConfigFrom(s: Pick<AppState, 'model' | 'vad' | 'src' | 'gloss'> & Partial<Pick<AppState, 'accel'>>): SpeechConfig {
  const languages = (['ru', 'en'] as const).filter((l) => s.src[l]);
  return {
    model: s.model,
    languages: languages.length ? [...languages] : ['ru', 'en'],
    vadThreshold: Math.min(0.9, Math.max(0.2, s.vad / 100)),
    glossary: s.gloss.map((g: GlossEntry) => g[0]),
    accel: s.accel ?? 'auto',
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
    time: now.toLocaleTimeString('fr-FR'),
    status: 'transcrit',
    shown: null,
    lat: t.partial ? null : t.latencyMs,
    err: false,
    live: t.partial,
  };
}

export interface EngineSummary {
  value: string;
  detail: string;
  color: string;
}

export function engineSummary(st: SpeechStatus | null): EngineSummary {
  if (!st) return { value: '…', detail: 'Moteur de transcription en cours de démarrage', color: '#8f8a83' };
  const model = `whisper ${st.config.model}`;
  switch (st.state) {
    case 'ready':
      return { value: `Whisper ${st.config.model}`, detail: `${st.backend} · ${st.threads} threads${st.loadMs != null ? ` · chargé en ${formatMs(st.loadMs)}` : ''}`, color: '#7fe0a8' };
    case 'loading':
      return { value: 'Chargement…', detail: `${model} · ${st.backend}`, color: '#ffa24a' };
    case 'error':
      return { value: 'Erreur', detail: st.error ?? 'Moteur indisponible', color: '#ff8a7a' };
    case 'missing':
      return {
        value: !st.vadReady ? 'VAD absente' : 'Modèle absent',
        detail: !st.vadReady ? 'ggml-silero-v5.1.2.bin manquant' : `${model} non installé`,
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
  noSpeech: 'pas de parole',
  lowConfidence: 'confiance faible',
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
