import { beforeEach, describe, expect, it } from 'vitest';
import type { GsiStatus } from '../../shared/ipc';
import { useGsi } from './gsi';
import { countEntry, finishMatch, formatDuration, median, newMatch, pickQuotes, recapFacts, startRecapTracker, useRecap, type MatchStats } from './recap';
import { useApp, type FeedEntry } from '../../app/state/store';

const line = (p: Partial<FeedEntry>): FeedEntry => ({ kind: 'voice', src: 'ru', orig: 'Двое на Б', prov: '', tr: null, id: 1, tid: 1, time: '', status: 'final', shown: 'Deux sur B.', lat: 1800, err: false, ...p });
const gsi = (p: Partial<GsiStatus>): GsiStatus => ({ installed: true, cfgPath: null, cs2Found: true, listening: true, port: 31982, error: null, map: 'de_mirage', mapName: 'Mirage', mode: 'competitive', callouts: 40, lastSeen: 1, ...p });

describe('match recap: counts', () => {
  it('counts each final line once, with its language, words and latency', () => {
    let m = newMatch(gsi({}), 0);
    m = countEntry(m, line({}));
    m = countEntry(m, line({})); // same line again
    m = countEntry(m, line({ tid: 2, src: 'en', orig: 'two B', shown: 'Deux en B, vite.', lat: 2200 }));
    m = countEntry(m, line({ tid: 2, kind: 'chat', src: 'fr', orig: 'je rotate', shown: null, status: 'original', lat: null })); // chat id 2 is another line
    m = countEntry(m, line({ tid: 3, status: 'transcrit', shown: null })); // not final yet
    m = countEntry(m, line({ tid: undefined })); // demo line
    expect(m).toMatchObject({ voice: 2, chat: 1, translated: 2, langs: { ru: 1, en: 1 }, words: 7, latencies: [1800, 2200] });
    expect(m.samples).toEqual(['Deux sur B.', 'Deux en B, vite.']);
  });

  it('writes the card text without any name', () => {
    const m: MatchStats = { ...newMatch(gsi({}), 0), endedAt: 34 * 60000, scoreCt: 13, scoreT: 9, voice: 74, chat: 14, translated: 87, langs: { en: 25, ru: 62 }, words: 412, latencies: [1700, 1900] };
    const f = recapFacts(m);
    expect(f.title).toBe('Mirage');
    expect(f.subtitle).toBe('Compétitif · 34 min · CT 13 – 9 T');
    expect(f.hero).toBe(87);
    expect(f.rows.map((r) => `${r.label}=${r.value}`)).toEqual(['Russe → français=62', 'Anglais → français=25', 'Messages du chat=14', 'Latence vocale médiane=1,8 s', 'Mots traduits=412']);
    expect(recapFacts({ ...newMatch(null, 0), translated: 1 }, 1000).title).toBe('Session CS2');
    expect(recapFacts({ ...newMatch(null, 0), translated: 1 }, 1000).heroLabel).toBe('annonce traduite');
  });

  it('helpers', () => {
    expect(median([3, 1, 2])).toBe(2);
    expect(median([1, 2])).toBe(2);
    expect(median([])).toBeNull();
    expect(formatDuration(20_000)).toBe('1 min');
    expect(formatDuration(95 * 60000)).toBe('1 h 35');
    const m = { ...newMatch(null, 0), samples: ['ok', 'Deux sur B.', 'Deux sur B.', 'Attention, smoke au Palace.', 'x'.repeat(90)] };
    expect(pickQuotes(m)).toEqual(['Deux sur B.', 'Attention, smoke au Palace.']);
  });

  it('a match with nothing translated gives no recap', () => {
    const s = { current: newMatch(gsi({}), 0), last: null, unseen: false };
    expect(finishMatch(s, null, 10)).toEqual({ current: null, last: null, unseen: false });
    const withLine = { ...s, current: countEntry(s.current, line({})) };
    const done = finishMatch(withLine, gsi({ scoreCt: 13, scoreT: 7 }), 10);
    expect(done.last).toMatchObject({ endedAt: 10, scoreCt: 13, scoreT: 7, voice: 1 });
    expect(done.unseen).toBe(true);
  });
});

describe('match recap: following CS2', () => {
  const app0 = useApp.getState();
  beforeEach(() => {
    useApp.setState(app0, true);
    useGsi.setState({ status: null });
    useRecap.setState({ current: null, last: null, unseen: false });
  });

  it('starts with the map, counts lines and is ready at game over', () => {
    const ready: string[] = [];
    const stop = startRecapTracker((m) => ready.push(m.mapName ?? '?'));
    useGsi.setState({ status: gsi({ phase: 'warmup' }) });
    expect(useRecap.getState().current?.map).toBe('de_mirage');
    useApp.setState({ feed: [line({})] });
    useApp.setState({ feed: [line({}), line({ tid: 2, src: 'en', shown: 'Rush B.' })] });
    useGsi.setState({ status: gsi({ phase: 'live', scoreCt: 5, scoreT: 3 }) });
    expect(useRecap.getState().current).toMatchObject({ translated: 2, scoreCt: 5 });
    useGsi.setState({ status: gsi({ phase: 'gameover', scoreCt: 13, scoreT: 9 }) });
    expect(ready).toEqual(['Mirage']);
    expect(useRecap.getState()).toMatchObject({ current: null, unseen: true, last: { translated: 2, scoreCt: 13, scoreT: 9 } });
    // Next match on the same map.
    useGsi.setState({ status: gsi({ phase: 'warmup', scoreCt: 0, scoreT: 0 }) });
    expect(useRecap.getState().current?.translated).toBe(0);
    stop();
  });

  it('without the map, a session is a match', () => {
    const stop = startRecapTracker();
    useApp.setState({ session: 'LISTENING' });
    useApp.setState({ feed: [line({})] });
    useApp.setState({ session: 'STOPPED' });
    expect(useRecap.getState().last).toMatchObject({ mapName: null, translated: 1 });
    stop();
  });
});
