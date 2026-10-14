import { afterEach, describe, expect, it } from 'vitest';
import type { HistoryEntry } from '../../shared/ipc';
import { historyRow } from './HistoryPage';
import { historyEntryFrom, startHistoryRecorder } from './history';
import { useApp, type FeedEntry } from '../../app/state/store';

const line = (over: Partial<FeedEntry>): FeedEntry => ({ id: 1, tid: 7, kind: 'voice', src: 'en', orig: 'rush B', tr: null, time: '12:00:00', status: 'final', shown: 'rush sur B', lat: 300, err: false, ...over });

describe('local history', () => {
  const initial = useApp.getState();
  afterEach(() => useApp.setState(initial, true));

  it('keeps only real lines in their final state', () => {
    expect(historyEntryFrom(line({}), 1000)).toEqual({ ts: 1000, kind: 'voice', lang: 'en', original: 'rush B', translated: 'rush sur B', status: 'final' });
    expect(historyEntryFrom(line({ tid: undefined }))).toBeNull(); // demo line
    expect(historyEntryFrom(line({ live: true }))).toBeNull();
    expect(historyEntryFrom(line({ status: 'provisoire' }))).toBeNull();
    expect(historyEntryFrom(line({ status: 'transcrit', shown: null }))).toBeNull(); // translation still coming
    expect(historyEntryFrom(line({ status: 'transcrit', shown: null, note: 'modèle absent' }))).toMatchObject({ status: 'transcrit' });
    expect(historyEntryFrom(line({ status: 'original', shown: 'salut', src: 'fr' }))).not.toHaveProperty('translated');
    const chat = historyEntryFrom(line({ kind: 'chat', ch: 'T', sender: 'Bob', status: 'final' }));
    expect(chat).toMatchObject({ kind: 'chat', channel: 'T', sender: 'Bob' });
  });

  it('records each line once, only while history is on', async () => {
    const got: HistoryEntry[] = [];
    const stop = startHistoryRecorder(async (e) => void got.push(e));
    useApp.setState({ feed: [line({})] });
    expect(got).toHaveLength(0); // off by default
    useApp.setState((s) => ({ tg: { ...s.tg, hist: true } }));
    useApp.setState({ feed: [line({ status: 'provisoire', live: true })] });
    useApp.setState({ feed: [line({})] });
    useApp.setState({ feed: [line({}), line({ id: 2, tid: 8, orig: 'eco', shown: 'éco' })] });
    expect(got.map((e) => e.original)).toEqual(['rush B', 'eco']);
    stop();
    useApp.setState({ feed: [line({ id: 3, tid: 9 })] });
    expect(got).toHaveLength(2);
  });

  it('shows stored lines honestly', () => {
    const base: HistoryEntry = { ts: Date.UTC(2026, 9, 13, 10), kind: 'chat', channel: 'CT', sender: 'Bob', lang: 'en', original: 'gg', status: 'original' };
    expect(historyRow(base)).toMatchObject({ original: 'Bob — gg', shown: 'Déjà dans votre langue', muted: true });
    expect(historyRow({ ...base, kind: 'voice', status: 'final', translated: 'bien joué' })).toMatchObject({ tag: 'VOIX · EN', shown: 'bien joué', muted: false });
    expect(historyRow({ ...base, status: 'transcrit' }).shown).toBe('Non traduit');
  });
});
