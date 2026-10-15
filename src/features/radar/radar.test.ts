import { describe, expect, it } from 'vitest';
import { LAYOUTS, lanes } from './layouts';
import { findZones, lineMentions, mentions } from './detect';
import { layoutOf, pingLife, pingsFrom, samplePings } from './radar';

const mirage = LAYOUTS.de_mirage;
const inferno = LAYOUTS.de_inferno;
const dust2 = LAYOUTS.de_dust2;
const zones = (text: string, m = mirage) => findZones(text, m).map((h) => h.zone);

describe('map plans', () => {
  it('covers the maps known to the game state integration, inside the square', () => {
    expect(Object.keys(LAYOUTS).sort()).toEqual(['de_ancient', 'de_anubis', 'de_dust2', 'de_inferno', 'de_mirage', 'de_nuke', 'de_overpass', 'de_train', 'de_vertigo']);
    for (const m of Object.values(LAYOUTS)) {
      const ids = m.zones.map((z) => z.id);
      expect(new Set(ids).size).toBe(ids.length);
      for (const common of ['a', 'b', 'mid', 'ct', 't']) expect(ids).toContain(common);
      for (const z of m.zones) {
        expect(z.x).toBeGreaterThanOrEqual(0);
        expect(z.x).toBeLessThanOrEqual(1);
        expect(z.y).toBeGreaterThanOrEqual(0);
        expect(z.y).toBeLessThanOrEqual(1);
        // A prefix shorter than 3 letters would match unrelated words.
        for (const a of z.aliases) for (const t of a.split(' ')) if (t.endsWith('*')) expect(t.length).toBeGreaterThan(3);
      }
      expect(lanes(m).length).toBeGreaterThan(m.zones.length / 2);
    }
    expect(layoutOf('de_mirage')?.name).toBe('Mirage');
    expect(layoutOf('cs_office')).toBeNull();
    expect(layoutOf(null)).toBeNull();
  });
});

describe('callouts in a line', () => {
  it('reads Russian callouts with their endings and the number of enemies', () => {
    expect(mentions('Двое на банане', inferno)).toEqual([{ zone: 'banana', count: 2 }]);
    expect(mentions('Двое на банане, один на машине', inferno)).toEqual([
      { zone: 'banana', count: 2 },
      { zone: 'car', count: 1 },
    ]);
    expect(mentions('трое идут на апы', mirage)).toEqual([{ zone: 'apps', count: 3 }]);
    expect(mentions('один в паласе, двое на рампе', mirage)).toEqual([
      { zone: 'palace', count: 1 },
      { zone: 'ramp', count: 2 },
    ]);
    expect(mentions('Двое идут на B, двое на B', mirage)).toEqual([{ zone: 'b', count: 2 }]);
    expect(mentions('Бомба на A, ротейт быстро', mirage)).toEqual([{ zone: 'a', count: null }]);
    expect(mentions('на а плент', mirage)).toEqual([{ zone: 'a', count: null }]);
    expect(mentions('топ мид чисто', mirage)).toEqual([{ zone: 'topmid', count: null }]);
  });

  it('reads English and chat shorthand', () => {
    expect(mentions('2 B', mirage)).toEqual([{ zone: 'b', count: 2 }]);
    expect(mentions('three apps', mirage)).toEqual([{ zone: 'apps', count: 3 }]);
    expect(mentions('rush A', mirage)).toEqual([{ zone: 'a', count: null }]);
    expect(mentions('A', mirage)).toEqual([{ zone: 'a', count: null }]);
    expect(mentions('one long, two short', dust2)).toEqual([
      { zone: 'longa', count: 1 },
      { zone: 'shorta', count: 2 },
    ]);
    expect(zones('top banana and car', inferno)).toEqual(['topbanana', 'car']);
    expect(zones('upper tunnels', dust2)).toEqual(['uppertunnels']);
  });

  it('does not take the article or a conjunction for a site', () => {
    expect(zones('he is at a corner')).toEqual([]);
    expect(zones('wait for a flash')).toEqual([]);
    expect(zones('Il en a deux')).toEqual([]);
    expect(zones('я жду, а ты иди')).toEqual([]);
    expect(zones('gg, хорошая игра')).toEqual([]);
    expect(zones('drop AWP pls')).toEqual([]);
  });

  it('merges the original and its translation, the original first', () => {
    expect(lineMentions('Двое на банане', 'Deux sur Banana', inferno)).toEqual([{ zone: 'banana', count: 2 }]);
    // Zone only in the translation (Whisper wrote it badly, the translation kept the callout).
    expect(lineMentions('двое там', 'Deux à Banana', inferno)).toEqual([{ zone: 'banana', count: 2 }]);
    expect(lineMentions('Двое идут на B', 'Deux joueurs arrivent sur B.', mirage)).toEqual([{ zone: 'b', count: 2 }]);
  });
});

describe('pings over time', () => {
  const now = 1_000_000;
  const line = (id: number, orig: string, shown: string | null = null, extra = {}) => ({ id, kind: 'voice' as const, orig, shown, ...extra });

  it('keeps the zones of the last seconds, newest first, and ignores live words', () => {
    const feed = [line(now - 2000, 'двое на апах'), line(now - 5000, 'один в паласе'), line(now - 20000, 'трое на B'), line(now - 500, 'на рампе', null, { live: true })];
    const pings = pingsFrom(feed, mirage, now, 12);
    expect(pings.map((p) => [p.zone, p.count])).toEqual([
      ['apps', 2],
      ['palace', 1],
    ]);
    expect(pingLife(pings[0], now, 12)).toBeCloseTo(1 - 2 / 12);
    expect(pingLife({ ...pings[0], at: now - 13000 }, now, 12)).toBe(0);
  });

  it('lights a zone again when it is named again', () => {
    const feed = [line(now - 1000, 'апы'), line(now - 3000, 'двое на апах')];
    const [p] = pingsFrom(feed, mirage, now, 12);
    expect(p).toMatchObject({ zone: 'apps', count: 2, at: now - 1000 });
  });

  it('has a sample for the preview', () => {
    const s = samplePings(now);
    expect(layoutOf(s.map)).not.toBeNull();
    for (const p of s.pings) expect(LAYOUTS[s.map].zones.some((z) => z.id === p.zone)).toBe(true);
  });
});
