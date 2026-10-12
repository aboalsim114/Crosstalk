// Pure view-model helpers shared by the pages (and covered by unit tests).

import { CHANNEL_NAME, LANG_NAME } from '../state/data';
import type { FeedEntry, FeedStatus } from '../state/store';
import { fmtSeconds } from '../components/atoms';

export const STATUS_COLOR: Record<FeedStatus, string> = {
  provisoire: '#ffa24a',
  final: '#7fe0a8',
  original: '#8f8a83',
  erreur: '#ff8a7a',
  transcrit: '#7fd4ff',
};

/** Shown under a real transcript while its French translation is computed. */
export const TRANSCRIPT_NOTE = 'Transcription en langue originale — traduction en cours';
export const CHAT_PENDING_NOTE = 'Message en langue originale — traduction en cours';

/**
 * In the desktop app every real line carries the id of its source (a voice
 * transcript or a console.log message): a line without one is a demo line.
 */
export function isDemoLine(native: boolean, f: FeedEntry | undefined): boolean {
  return native && !!f && f.tid == null;
}

/** Under a transcript that stays untranslated, with the reason. */
/** Advice when the measured voice delay is too long for live subtitles. */
export function latencyAdvice(p50Ms: number, model: string): string | null {
  if (p50Ms <= 3000) return null;
  if (model !== 'tiny' && model !== 'base') return `Trop lent pour le direct avec whisper-${model} : choisissez whisper-base (Audio & IA)`;
  return 'Délai élevé : le processeur est très chargé pendant la partie';
}

export const LIVE_NOTE = 'En cours d’écoute… le texte se complète';
export const PROVISIONAL_NOTE = 'Traduction provisoire — version soignée en cours';

export function transcriptNote(f: FeedEntry): string {
  if (f.live) return f.shown ? `${LIVE_NOTE} · traduction provisoire` : LIVE_NOTE;
  if (f.shown && !f.note) return PROVISIONAL_NOTE;
  if (f.note) return `Non traduit — ${f.note}`;
  return f.kind === 'chat' ? CHAT_PENDING_NOTE : TRANSCRIPT_NOTE;
}

export interface FeedRow {
  id: number;
  tag: string;
  color: string;
  time: string;
  langs: string;
  lead: string;
  orig: string;
  shown: string;
  trColor: string;
  italic: boolean;
  note: string;
  noteColor: string;
  status: string;
  statusColor: string;
  lat: string;
}

export function feedRow(f: FeedEntry): FeedRow {
  if (f.status === 'transcrit') {
    const voice = f.kind === 'voice';
    return {
      id: f.id,
      tag: voice ? 'VOIX' : 'CHAT · ' + CHANNEL_NAME[f.ch!],
      color: voice ? '#ffa24a' : '#7fd4ff',
      time: f.time,
      langs: (f.src === 'xx' ? '??' : f.src.toUpperCase()) + (f.shown ? ' → FR · PROVISOIRE' : voice ? ' · TRANSCRIPTION' : ' · ORIGINAL'),
      lead: f.sender ? f.sender + ' — ' : '',
      // Provisional French (live line) above its original, else the original alone.
      orig: f.shown ? f.orig : '',
      shown: (f.shown ?? f.orig) + (f.live ? ' …' : ''),
      trColor: f.live || f.shown ? '#b9b2a7' : '#ece6dc',
      italic: !!f.live || !!f.shown,
      note: transcriptNote(f),
      noteColor: '#6e6961',
      status: f.live ? 'EN DIRECT' : 'TRANSCRIT',
      statusColor: STATUS_COLOR.transcrit,
      lat: f.lat ? fmtSeconds(f.lat) : '',
    };
  }
  const voice = f.kind === 'voice';
  const fr = f.status === 'original';
  const prov = f.status === 'provisoire';
  return {
    id: f.id,
    tag: voice ? 'VOIX' : 'CHAT · ' + f.ch,
    color: voice ? '#ffa24a' : '#7fd4ff',
    time: f.time,
    langs: f.src.toUpperCase() + (fr ? '' : ' → FR'),
    lead: f.sender ? f.sender + ' — ' : '',
    orig: fr || f.err ? '' : f.orig,
    shown: f.shown || f.orig,
    trColor: prov ? '#8f8a83' : '#ece6dc',
    italic: prov,
    note: fr ? 'Déjà en français — non traduit' : f.err ? `Moteur indisponible — original affiché${f.note ? ` (${f.note})` : ''}` : '',
    noteColor: f.err ? '#ff8a7a' : '#5a5650',
    status: f.status.toUpperCase(),
    statusColor: STATUS_COLOR[f.status],
    lat: f.lat ? fmtSeconds(f.lat) : '',
  };
}

export interface Subtitle {
  tag: string;
  sender: string;
  color: string;
  orig: string;
  text: string;
  italic: boolean;
  trColor: string;
  note: string;
  noteColor: string;
}

/** The big film-style subtitle on the dashboard: latest line, or an idle message. */
export function subtitle(f0: FeedEntry | undefined, live: boolean): Subtitle {
  if (!f0)
    return {
      tag: live ? 'EN ÉCOUTE' : 'HORS LIGNE',
      sender: '',
      color: '#5a5650',
      orig: '',
      text: live ? 'En attente d’une voix…' : 'Le silence, pour l’instant.',
      italic: true,
      trColor: '#6e6961',
      note: live ? '' : 'Démarrez la session pour sous-titrer la partie.',
      noteColor: '#5a5650',
    };
  if (f0.status === 'transcrit') {
    const voice = f0.kind === 'voice';
    return {
      tag: (voice ? 'VOIX' : 'CHAT · ' + CHANNEL_NAME[f0.ch!]) + ' · ' + LANG_NAME[f0.src] + (voice ? ' · TRANSCRIPTION' : ' · ORIGINAL'),
      sender: f0.sender ?? '',
      color: voice ? '#ffa24a' : '#7fd4ff',
      orig: f0.shown ? f0.orig : '',
      text: '« ' + (f0.shown ?? f0.orig) + (f0.live ? ' …' : '') + ' »',
      italic: !!f0.live || !!f0.shown,
      trColor: f0.live || f0.shown ? '#c9c2b7' : '#f3ede3',
      note: transcriptNote(f0),
      noteColor: '#6e6961',
    };
  }
  const fr = f0.status === 'original';
  const prov = f0.status === 'provisoire';
  return {
    tag: (f0.kind === 'voice' ? 'VOIX' : 'CHAT · ' + CHANNEL_NAME[f0.ch!]) + ' · ' + LANG_NAME[f0.src] + (fr ? '' : ' → FRANÇAIS'),
    sender: f0.sender || '',
    color: f0.kind === 'voice' ? '#ffa24a' : '#7fd4ff',
    orig: fr ? '' : f0.orig,
    text: '« ' + (f0.shown || f0.orig) + ' »',
    italic: prov,
    trColor: prov ? '#a49d92' : '#f3ede3',
    note: prov ? 'Transcription provisoire — correction en cours' : f0.err ? 'Moteur de traduction indisponible — original affiché' : '',
    noteColor: f0.err ? '#ff8a7a' : '#6e6961',
  };
}

/** Voice latency percentiles (p50/p95) over final voice lines. */
export function voiceLatency(feed: FeedEntry[]): { p50: number; p95: number; n: number } | null {
  const vl = feed
    .filter((f) => f.kind === 'voice' && f.lat)
    .map((f) => f.lat!)
    .sort((a, b) => a - b);
  if (!vl.length) return null;
  const pct = (q: number) => vl[Math.min(vl.length - 1, Math.floor(q * vl.length))];
  return { p50: pct(0.5), p95: pct(0.95), n: vl.length };
}
