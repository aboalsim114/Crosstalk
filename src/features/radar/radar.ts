// Callout radar: recent lines → zones lit on the map plan.

import { LAYOUTS, type MapLayout } from './layouts';
import { lineMentions } from './detect';

export interface Ping {
  zone: string;
  count: number | null;
  /** Arrival time of the line (ms). */
  at: number;
  kind: 'voice' | 'chat';
}

/** What the radar needs of a feed line. */
export interface RadarLine {
  id: number;
  kind: 'voice' | 'chat';
  orig: string;
  shown: string | null;
  live?: boolean;
}

export interface RadarOpts {
  on: boolean;
  /** Seconds a zone stays lit. */
  hold: number;
}

export const RADAR_DEFAULTS: RadarOpts = { on: true, hold: 12 };
export const HOLD_RANGE: [number, number] = [6, 20];

export function layoutOf(map: string | null | undefined): MapLayout | null {
  return (map && LAYOUTS[map]) || null;
}

/**
 * Lit zones from the lines of the last `hold` seconds, newest first. A zone
 * named again is lit again; the latest count said for it is kept.
 */
export function pingsFrom(feed: RadarLine[], m: MapLayout, now: number, hold: number): Ping[] {
  const byZone = new Map<string, Ping>();
  // Oldest first, so that newer lines overwrite.
  const recent = feed.filter((f) => !f.live && now - f.id < hold * 1000 && f.id <= now).sort((p, q) => p.id - q.id);
  for (const f of recent) {
    for (const m2 of lineMentions(f.orig, f.shown, m)) {
      const prev = byZone.get(m2.zone);
      byZone.set(m2.zone, {
        zone: m2.zone,
        count: m2.count ?? (prev && now - prev.at < 4000 ? prev.count : null),
        at: f.id,
        kind: f.kind,
      });
    }
  }
  return [...byZone.values()].sort((p, q) => q.at - p.at);
}

/** 1 when the ping is new, down to 0 at the end of `hold`. */
export function pingLife(p: Ping, now: number, hold: number): number {
  return Math.max(0, Math.min(1, 1 - (now - p.at) / (hold * 1000)));
}

/** Sample shown while repositioning the overlay and in the settings preview. */
export function samplePings(now: number): { map: string; pings: Ping[] } {
  return {
    map: 'de_mirage',
    pings: [
      { zone: 'apps', count: 2, at: now - 1500, kind: 'voice' },
      { zone: 'palace', count: 1, at: now - 5000, kind: 'chat' },
    ],
  };
}

/** What the overlay window receives: the map and its lit zones. */
export interface RadarPayload {
  map: string;
  pings: Ping[];
  hold: number;
}

/**
 * Radar state for the overlay: nothing when the radar is off or the map has no
 * plan; the sample on Mirage for a few seconds after "Essayer".
 */
export function radarPayload(opts: RadarOpts, map: string | null | undefined, feed: RadarLine[], demo: number | null, now: number): RadarPayload | null {
  if (!opts.on) return null;
  if (demo !== null && now - demo < opts.hold * 1000) {
    const s = samplePings(demo);
    return { map: s.map, pings: s.pings, hold: opts.hold };
  }
  const layout = layoutOf(map);
  if (!layout) return null;
  return {
    map: layout.id,
    pings: pingsFrom(feed, layout, now, opts.hold),
    hold: opts.hold,
  };
}
