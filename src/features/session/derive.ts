// Pure view-model helpers shared by the pages (and covered by unit tests).

import { channelName, langName, quote, t, targetName, useTargetLang } from '../../shared/i18n';
import type { FeedEntry, FeedStatus } from '../../app/state/store';
import { fmtSeconds } from '../../shared/ui/atoms';

export const STATUS_COLOR: Record<FeedStatus, string> = {
  provisoire: '#ffa24a',
  final: '#7fe0a8',
  original: '#8f8a83',
  erreur: '#ff8a7a',
  transcrit: '#7fd4ff',
};


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
  if (model !== 'tiny' && model !== 'base') return t('advice.slowModel', { model });
  return t('advice.cpu');
}

export function transcriptNote(f: FeedEntry): string {
  if (f.live) return f.shown ? t('note.liveProvisional') : t('note.live');
  if (f.shown && !f.note) return t('note.provisional');
  if (f.note) return t('note.untranslated', { reason: f.note });
  return f.kind === 'chat' ? t('note.chatPending') : t('note.transcript');
}

/** "VOIX" or "CHAT · ÉQUIPE CT" in the interface language. */
function kindTag(f: FeedEntry): string {
  return f.kind === 'voice' ? t('feed.voice') : `${t('feed.chat')} · ${channelName(f.ch)}`;
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

/** Code of the translations' language ("FR", "RU"…). */
const targetCode = () => useTargetLang.getState().target.toUpperCase();

export function feedRow(f: FeedEntry): FeedRow {
  if (f.status === 'transcrit') {
    const voice = f.kind === 'voice';
    return {
      id: f.id,
      tag: kindTag(f),
      color: voice ? '#ffa24a' : '#7fd4ff',
      time: f.time,
      langs: (f.src === 'xx' ? '??' : f.src.toUpperCase()) + (f.shown ? ` → ${targetCode()} · ${t('feed.provisional')}` : ` · ${voice ? t('feed.transcription') : t('feed.original')}`),
      lead: f.sender ? f.sender + ' — ' : '',
      // Provisional French (live line) above its original, else the original alone.
      orig: f.shown ? f.orig : '',
      shown: (f.shown ?? f.orig) + (f.live ? ' …' : ''),
      trColor: f.live || f.shown ? '#b9b2a7' : '#ece6dc',
      italic: !!f.live || !!f.shown,
      note: transcriptNote(f),
      noteColor: '#6e6961',
      status: f.live ? t('feed.live') : t('status.transcrit'),
      statusColor: STATUS_COLOR.transcrit,
      lat: f.lat ? fmtSeconds(f.lat) : '',
    };
  }
  const voice = f.kind === 'voice';
  const fr = f.status === 'original';
  const prov = f.status === 'provisoire';
  return {
    id: f.id,
    tag: voice ? t('feed.voice') : `${t('feed.chat')} · ${f.ch}`,
    color: voice ? '#ffa24a' : '#7fd4ff',
    time: f.time,
    langs: f.src.toUpperCase() + (fr ? '' : ` → ${targetCode()}`),
    lead: f.sender ? f.sender + ' — ' : '',
    orig: fr || f.err ? '' : f.orig,
    shown: f.shown || f.orig,
    trColor: prov ? '#8f8a83' : '#ece6dc',
    italic: prov,
    note: fr ? t('note.alreadyTarget') : f.err ? (f.note ? t('note.engineDownWhy', { reason: f.note }) : t('note.engineDown')) : (f.note ?? ''),
    noteColor: f.err ? '#ff8a7a' : '#5a5650',
    status: t(`status.${f.status}`),
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
      tag: live ? t('sub.listening') : t('sub.offline'),
      sender: '',
      color: '#5a5650',
      orig: '',
      text: live ? t('sub.waiting') : t('sub.silence'),
      italic: true,
      trColor: '#6e6961',
      note: live ? '' : t('sub.start'),
      noteColor: '#5a5650',
    };
  if (f0.status === 'transcrit') {
    const voice = f0.kind === 'voice';
    return {
      tag: `${kindTag(f0)} · ${langName(f0.src)} · ${voice ? t('feed.transcription') : t('feed.original')}`,
      sender: f0.sender ?? '',
      color: voice ? '#ffa24a' : '#7fd4ff',
      orig: f0.shown ? f0.orig : '',
      text: quote((f0.shown ?? f0.orig) + (f0.live ? ' …' : '')),
      italic: !!f0.live || !!f0.shown,
      trColor: f0.live || f0.shown ? '#c9c2b7' : '#f3ede3',
      note: transcriptNote(f0),
      noteColor: '#6e6961',
    };
  }
  const fr = f0.status === 'original';
  const prov = f0.status === 'provisoire';
  return {
    tag: `${kindTag(f0)} · ${langName(f0.src)}${fr ? '' : ` → ${targetName()}`}`,
    sender: f0.sender || '',
    color: f0.kind === 'voice' ? '#ffa24a' : '#7fd4ff',
    orig: fr ? '' : f0.orig,
    text: quote(f0.shown || f0.orig),
    italic: prov,
    trColor: prov ? '#a49d92' : '#f3ede3',
    note: prov ? t('note.provTranscript') : f0.err ? t('note.trDown') : (f0.note ?? ''),
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
