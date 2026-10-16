// Callout radar: finds the zones a teammate names in a line ("двое на банане",
// "2 B", "one apps") and the number of enemies said with them. Pure functions,
// run on the original line and on its translation.

import type { MapLayout, Zone } from './layouts';

export interface Mention {
  zone: string;
  /** Enemies announced at the zone, when a number is said with it. */
  count: number | null;
}

/** Words as written: letters and digits, everything else is a separator. */
function rawWords(text: string): string[] {
  return text.split(/[^\p{L}\p{N}]+/u).filter(Boolean);
}

/** Lowercase words, `ё` → `е`. */
export function words(text: string): string[] {
  return rawWords(text).map((w) => w.toLowerCase().replace(/ё/g, 'е'));
}

const NUMBERS: Record<string, number> = {
  '1': 1,
  '2': 2,
  '3': 3,
  '4': 4,
  '5': 5,
  one: 1,
  two: 2,
  three: 3,
  four: 4,
  five: 5,
  один: 1,
  одного: 1,
  одна: 1,
  одну: 1,
  два: 2,
  две: 2,
  двое: 2,
  двоих: 2,
  двух: 2,
  три: 3,
  трое: 3,
  троих: 3,
  трех: 3,
  четыре: 4,
  четверо: 4,
  четверых: 4,
  пять: 5,
  пятеро: 5,
  пятерых: 5,
  // French "un/une" is left out: it is also the article.
  deux: 2,
  trois: 3,
  quatre: 4,
  cinq: 5,
};

/** Words before a site letter that make it a place: "на А", "go B", "sur A". */
// Not French "en": "il en a deux" is not a site.
const BEFORE_SITE = new Set(['на', 'в', 'к', 'on', 'at', 'to', 'go', 'in', 'rush', 'push', 'sur', 'vers', 'го', 'гоу', 'раш', 'пуш', 'site', 'bombsite', 'плент', 'плэнт', 'плант', 'точка', 'точку', 'point']);
const AFTER_SITE = new Set(['site', 'plant', 'сайт', 'плент', 'плэнт', 'плант', 'точка', 'точку', 'long', 'short', 'main', 'лонг', 'шорт', 'мейн']);
const SHORT_LINE_WORDS = new Set(['go', 'rush', 'push', 'го', 'гоу', 'раш', 'пуш', 'all', 'все']);
const SITE_LETTERS: Record<string, 'a' | 'b'> = {
  a: 'a',
  а: 'a',
  эй: 'a',
  b: 'b',
  б: 'b',
  би: 'b',
  бэ: 'b',
};
/** Letters that are never a word: always a site. */
const ALWAYS_SITE = new Set(['b', 'би', 'бэ']);

function tokenMatches(pattern: string, word: string): boolean {
  return pattern.endsWith('*') ? word.startsWith(pattern.slice(0, -1)) && word.length >= pattern.length - 1 : word === pattern;
}

interface Hit {
  zone: string;
  start: number;
  end: number;
}

interface Pattern {
  zone: string;
  tokens: string[];
}

const patternCache = new WeakMap<MapLayout, Pattern[]>();

/** Alias patterns of a map, longest first (so "top banana" wins over "banana"). */
function patterns(m: MapLayout): Pattern[] {
  let p = patternCache.get(m);
  if (!p) {
    p = m.zones
      .flatMap((z: Zone) =>
        z.aliases.map((a) => ({
          zone: z.id,
          tokens: a.split(/\s+/).filter(Boolean),
        })),
      )
      .filter((x) => x.tokens.length > 0)
      .sort((x, y) => y.tokens.length - x.tokens.length);
    patternCache.set(m, p);
  }
  return p;
}

function siteAt(w: string[], raw: string[], i: number): 'a' | 'b' | null {
  const site = SITE_LETTERS[w[i]];
  if (!site) return null;
  if (ALWAYS_SITE.has(w[i])) return site;
  const prev = w[i - 1];
  const next = w[i + 1];
  if (next && AFTER_SITE.has(next)) return site;
  if (prev && BEFORE_SITE.has(prev)) {
    // English "a" is mostly the article ("at a corner"): only the capital
    // letter, or "a" at the end of the line or before a number, is the site.
    if (raw[i] !== 'a') return site;
    if (!next || NUMBERS[next] !== undefined || SHORT_LINE_WORDS.has(next)) return site;
    return null;
  }
  // A whole short line: "A", "2 A", "A go".
  if (w.length <= 3 && w.every((x, j) => j === i || NUMBERS[x] !== undefined || SHORT_LINE_WORDS.has(x))) return site;
  return null;
}

/** Zones named in a text, in order, without duplicates. */
export function findZones(text: string, m: MapLayout): Hit[] {
  const raw = rawWords(text);
  const w = words(text);
  const pats = patterns(m);
  const hits: Hit[] = [];
  for (let i = 0; i < w.length; ) {
    let hit: Hit | null = null;
    for (const p of pats) {
      if (i + p.tokens.length > w.length) continue;
      if (p.tokens.every((t, k) => tokenMatches(t, w[i + k]))) {
        hit = { zone: p.zone, start: i, end: i + p.tokens.length };
        break;
      }
    }
    if (!hit) {
      const site = siteAt(w, raw, i);
      if (site) hit = { zone: site, start: i, end: i + 1 };
    }
    if (hit) {
      hits.push(hit);
      i = hit.end;
    } else i++;
  }
  return hits;
}

/** Zones and counts of one text: a number belongs to the zone right after (or before) it. */
export function mentions(text: string, m: MapLayout): Mention[] {
  const w = words(text);
  const hits = findZones(text, m);
  const used = new Set<number>();
  const numberAt = (i: number) => (i >= 0 && i < w.length && !used.has(i) ? NUMBERS[w[i]] : undefined);
  const out: Mention[] = [];
  for (const h of hits) {
    let count: number | null = null;
    // "двое на банане": the number comes up to three words before the zone.
    for (let i = h.start - 1; i >= Math.max(0, h.start - 3) && count === null; i--) {
      const n = numberAt(i);
      if (n !== undefined) {
        count = n;
        used.add(i);
      }
    }
    // "B two", "банан двое".
    for (let i = h.end; i < Math.min(w.length, h.end + 2) && count === null; i++) {
      const n = numberAt(i);
      if (n !== undefined) {
        count = n;
        used.add(i);
      }
    }
    const prev = out.find((o) => o.zone === h.zone);
    if (prev) prev.count = prev.count ?? count;
    else out.push({ zone: h.zone, count });
  }
  return out;
}

/**
 * Mentions of a line from its original text and its translation: the original
 * wins (it is what was said), the translation adds zones the recogniser wrote in Latin
 * letters or that the translation kept ("Banana").
 */
export function lineMentions(orig: string, translated: string | null, m: MapLayout): Mention[] {
  const out = mentions(orig, m);
  if (translated) {
    for (const t of mentions(translated, m)) {
      const same = out.find((o) => o.zone === t.zone);
      if (!same) out.push(t);
      else if (same.count === null) same.count = t.count;
    }
  }
  return out;
}
