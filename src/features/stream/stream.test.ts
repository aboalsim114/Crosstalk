import { afterEach, describe, expect, it } from 'vitest';
import type { StreamLine, StreamStatus } from '../../shared/ipc';
import { streamSummary } from './StreamCard';
import { restore } from '../../app/state/persist';
import { startStreamBridge, streamLineFrom } from './stream';
import { STREAM_DEFAULTS, useApp, type FeedEntry } from '../../app/state/store';

const line = (over: Partial<FeedEntry>): FeedEntry => ({ id: 1, tid: 7, kind: 'voice', src: 'ru', orig: 'Двое на Б', tr: null, time: '12:00:00', status: 'final', shown: 'Deux sur B', lat: 300, err: false, ...over });
const st = (over: Partial<StreamStatus>): StreamStatus => ({ options: { ...STREAM_DEFAULTS, enabled: true }, listening: true, url: 'http://127.0.0.1:31983/', clients: 0, sent: 0, error: null, ...over });

describe('mode streamer (OBS)', () => {
  const initial = useApp.getState();
  afterEach(() => useApp.setState(initial, true));

  it('sends only translated or French final lines', () => {
    expect(streamLineFrom(line({}))).toEqual({ kind: 'voice', lang: 'ru', original: 'Двое на Б', translated: 'Deux sur B' });
    expect(streamLineFrom(line({ status: 'original', src: 'fr', orig: 'on rush B' }))).toEqual({ kind: 'voice', lang: 'fr', original: 'on rush B' });
    expect(streamLineFrom(line({ status: 'transcrit', shown: null, note: 'modèle absent' }))).toBeNull(); // foreign text, no translation
    expect(streamLineFrom(line({ live: true }))).toBeNull();
    expect(streamLineFrom(line({ tid: undefined }))).toBeNull();
  });

  it('pushes each line once, only while the mode is on', () => {
    const got: StreamLine[] = [];
    const stop = startStreamBridge(async (l) => (got.push(l), true));
    useApp.setState({ feed: [line({})] });
    expect(got).toHaveLength(0);
    useApp.getState().setStream({ enabled: true });
    useApp.setState({ feed: [line({})] });
    useApp.setState({ feed: [line({}), line({ id: 2, tid: 8, orig: 'eco', src: 'en', shown: 'éco' })] });
    expect(got.map((l) => l.translated)).toEqual(['Deux sur B', 'éco']);
    stop();
  });

  it('keeps its settings between launches, names off by default', () => {
    expect(STREAM_DEFAULTS).toMatchObject({ enabled: false, names: false, original: false, watermark: true });
    const r = restore({ version: 1, settings: { stream: { enabled: true, size: 48, names: 'yes', bogus: 1 } } }, useApp.getState());
    expect(r.app.stream).toEqual({ ...STREAM_DEFAULTS, enabled: true, size: 48 });
  });

  it('says whether OBS is connected', () => {
    expect(streamSummary(null, false).value).toBe('Désactivé');
    expect(streamSummary(st({}), true).value).toBe('Prêt · en attente d’OBS');
    expect(streamSummary(st({ clients: 1, sent: 3 }), true)).toMatchObject({ value: 'OBS connecté', detail: '3 sous-titres envoyés depuis le lancement.' });
    expect(streamSummary(st({ error: 'port local 31983 indisponible' }), true).value).toBe('Indisponible');
  });
});
