// End-of-match recap: what Crosstalk did during a match (lines translated,
// languages, latency), shown as a card the user can save or copy and post.
// A match follows CS2's Game State Integration (map, phase, score); without
// it, a session (Démarrer → Arrêter) counts as a match. Counts only: no
// player name, and the translated lines are left out unless the user adds
// them to the card.

import { create } from 'zustand';
import type { GsiStatus } from '../../shared/ipc';
import { useGsi } from './gsi';
import { historyEntryFrom } from '../history/history';
import { isLive, useApp, type FeedEntry } from '../../app/state/store';
import { layoutOf } from '../radar/radar';
import { lineMentions } from '../radar/detect';

/** Translated lines kept for the optional quotes on the card. */
const MAX_SAMPLES = 40;

export interface MatchStats {
  map: string | null;
  mapName: string | null;
  mode: string | null;
  startedAt: number;
  endedAt: number | null;
  scoreCt: number | null;
  scoreT: number | null;
  /** Final lines (translated, already in French, or left as is). */
  voice: number;
  chat: number;
  /** Lines with a real translation into French. */
  translated: number;
  /** Source language of the translated lines. */
  langs: Record<string, number>;
  /** French words produced. */
  words: number;
  /** Voice latency of the translated lines (ms). */
  latencies: number[];
  /** Last translated lines, French only (no names). */
  samples: string[];
  /** `kind:tid` of the counted lines. */
  counted: string[];
  /** Callouts heard per zone of the map plan (lines naming it), for the heat map. */
  zones: Record<string, number>;
}

export function newMatch(gsi: Pick<GsiStatus, 'map' | 'mapName' | 'mode'> | null, now = Date.now()): MatchStats {
  return {
    map: gsi?.map ?? null,
    mapName: gsi?.mapName ?? null,
    mode: gsi?.mode ?? null,
    startedAt: now,
    endedAt: null,
    scoreCt: null,
    scoreT: null,
    voice: 0,
    chat: 0,
    translated: 0,
    langs: {},
    words: 0,
    latencies: [],
    samples: [],
    counted: [],
    zones: {},
  };
}

/** Counts a feed line once it is final; the same match if nothing new. */
export function countEntry(m: MatchStats, f: FeedEntry): MatchStats {
  if (f.tid == null) return m;
  const key = `${f.kind}:${f.tid}`;
  if (m.counted.includes(key)) return m;
  const e = historyEntryFrom(f);
  if (!e) return m;
  const next: MatchStats = { ...m, counted: [...m.counted, key] };
  if (f.kind === 'voice') next.voice++;
  else next.chat++;
  const layout = layoutOf(m.map);
  if (layout) {
    const named = lineMentions(e.original, e.translated ?? null, layout);
    if (named.length) {
      next.zones = { ...m.zones };
      for (const z of named) next.zones[z.zone] = (next.zones[z.zone] ?? 0) + 1;
    }
  }
  if (e.translated) {
    next.translated++;
    next.langs = { ...m.langs, [e.lang]: (m.langs[e.lang] ?? 0) + 1 };
    next.words += e.translated.split(/\s+/).filter(Boolean).length;
    next.samples = [...m.samples, e.translated].slice(-MAX_SAMPLES);
    if (f.kind === 'voice' && f.lat != null) next.latencies = [...m.latencies, f.lat];
  }
  return next;
}

export function median(v: number[]): number | null {
  if (!v.length) return null;
  const s = [...v].sort((a, b) => a - b);
  const mid = s.length >> 1;
  return s.length % 2 ? s[mid] : Math.round((s[mid - 1] + s[mid]) / 2);
}

const LANG_FR: Record<string, string> = { ru: 'Russe', en: 'Anglais', de: 'Allemand', es: 'Espagnol', pt: 'Portugais', pl: 'Polonais', uk: 'Ukrainien', tr: 'Turc' };
const MODE_FR: Record<string, string> = { competitive: 'Compétitif', premier: 'Premier', casual: 'Occasionnel', deathmatch: 'Combat à mort', wingman: 'Duo', scrimcomp2v2: 'Duo', gungameprogressive: 'Course aux armements' };

export interface RecapFacts {
  title: string;
  subtitle: string;
  hero: number;
  heroLabel: string;
  rows: { label: string; value: string }[];
}

export function formatDuration(ms: number): string {
  const min = Math.max(1, Math.round(ms / 60000));
  return min >= 60 ? `${Math.floor(min / 60)} h ${String(min % 60).padStart(2, '0')}` : `${min} min`;
}

/** The text of the card. */
export function recapFacts(m: MatchStats, now = Date.now()): RecapFacts {
  const parts = [m.mode ? (MODE_FR[m.mode] ?? m.mode) : null, formatDuration((m.endedAt ?? now) - m.startedAt), m.scoreCt != null && m.scoreT != null ? `CT ${m.scoreCt} – ${m.scoreT} T` : null].filter(Boolean);
  const rows = Object.entries(m.langs)
    .sort((a, b) => b[1] - a[1])
    .slice(0, 3)
    .map(([l, n]) => ({ label: `${LANG_FR[l] ?? l.toUpperCase()} → français`, value: String(n) }));
  if (m.chat) rows.push({ label: 'Messages du chat', value: String(m.chat) });
  const lat = median(m.latencies);
  if (lat != null) rows.push({ label: 'Latence vocale médiane', value: `${(lat / 1000).toFixed(1).replace('.', ',')} s` });
  if (m.words) rows.push({ label: 'Mots traduits', value: String(m.words) });
  return {
    title: m.mapName ?? 'Session CS2',
    subtitle: parts.join(' · '),
    hero: m.translated,
    heroLabel: m.translated === 1 ? 'annonce traduite' : 'annonces traduites',
    rows: rows.slice(0, 5),
  };
}

/** The most called zones, most first (ties: the plan's order). */
export function topZones(m: Pick<MatchStats, 'map' | 'zones'>, n = 3): { id: string; label: string; count: number }[] {
  const layout = layoutOf(m.map);
  if (!layout) return [];
  return layout.zones
    .map((z, i) => ({ id: z.id, label: z.label, count: m.zones?.[z.id] ?? 0, i }))
    .filter((z) => z.count > 0)
    .sort((a, b) => b.count - a.count || a.i - b.i)
    .slice(0, n)
    .map(({ id, label, count }) => ({ id, label, count }));
}

/** Up to `n` short translated lines for the optional quotes. */
export function pickQuotes(m: MatchStats, n = 3): string[] {
  const seen = new Set<string>();
  return m.samples
    .filter((s) => s.length >= 8 && s.length <= 70)
    .filter((s) => !seen.has(s) && !!seen.add(s))
    .slice(-n);
}

// ---------- live tracking ----------

export interface RecapState {
  current: MatchStats | null;
  /** The last finished match with at least one line. */
  last: MatchStats | null;
  /** A new recap the user has not opened yet. */
  unseen: boolean;
}

export const useRecap = create<RecapState>()(() => ({ current: null, last: null, unseen: false }));

const hasLines = (m: MatchStats | null): m is MatchStats => !!m && m.voice + m.chat > 0;

/** Ends the current match; it becomes the recap if anything was translated. */
export function finishMatch(s: RecapState, gsi: GsiStatus | null, now = Date.now()): RecapState {
  const cur = s.current;
  if (!cur) return s;
  const ended: MatchStats = { ...cur, endedAt: now, scoreCt: gsi?.scoreCt ?? cur.scoreCt, scoreT: gsi?.scoreT ?? cur.scoreT };
  return hasLines(ended) ? { current: null, last: ended, unseen: true } : { ...s, current: null };
}

/** Follows the map, the match phase and the session; counts final lines. */
export function startRecapTracker(onReady: (m: MatchStats) => void = () => {}): () => void {
  const finish = () => {
    const before = useRecap.getState();
    const after = finishMatch(before, useGsi.getState().status);
    useRecap.setState(after);
    if (after.last && after.last !== before.last) onReady(after.last);
  };
  const stopGsi = useGsi.subscribe((g, prev) => {
    const st = g.status;
    const was = prev.status;
    const cur = useRecap.getState().current;
    if (st?.phase === 'gameover' && was?.phase !== 'gameover') return finish();
    if (st?.map !== was?.map) {
      if (cur && cur.map !== st?.map) finish();
      if (st?.map) useRecap.setState({ current: newMatch(st) });
      return;
    }
    // A new match on the same map (after a game over).
    if (st?.map && !useRecap.getState().current && (st.phase === 'warmup' || st.phase === 'live')) useRecap.setState({ current: newMatch(st) });
    if (cur && st && (st.scoreCt != null || st.scoreT != null)) useRecap.setState({ current: { ...cur, scoreCt: st.scoreCt ?? null, scoreT: st.scoreT ?? null } });
  });
  const stopApp = useApp.subscribe((s, prev) => {
    const live = isLive(s) || s.session === 'PAUSED';
    const wasLive = isLive(prev) || prev.session === 'PAUSED';
    // Without CS2's map, a session is a match.
    if (live && !wasLive && !useRecap.getState().current) useRecap.setState({ current: newMatch(useGsi.getState().status) });
    if (!live && wasLive && !useGsi.getState().status?.map) finish();
    if (s.feed === prev.feed) return;
    let cur = useRecap.getState().current;
    if (!cur) return;
    for (const f of s.feed) cur = countEntry(cur, f);
    if (cur !== useRecap.getState().current) useRecap.setState({ current: cur });
  });
  return () => {
    stopGsi();
    stopApp();
  };
}
