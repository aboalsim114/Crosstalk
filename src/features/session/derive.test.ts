import { describe, expect, it } from 'vitest';
import type { FeedEntry } from '../../app/state/store';
import { timecode } from '../../app/shell/TitleBar';
import { sceneTarget } from '../../app/scene/HeadsetScene';
import { overlayBlocks } from '../overlay/OverlayPage';
import { feedRow, subtitle, voiceLatency } from './derive';

const voice = (over: Partial<FeedEntry> = {}): FeedEntry => ({
  kind: 'voice',
  src: 'ru',
  orig: 'Бомба на A',
  tr: 'Bombe sur A.',
  id: 1,
  time: '14:00:00',
  status: 'final',
  shown: 'Bombe sur A.',
  lat: 1500,
  err: false,
  ...over,
});
const chat = (over: Partial<FeedEntry> = {}): FeedEntry => ({
  kind: 'chat',
  ch: 'CT',
  sender: 'Lucas',
  src: 'fr',
  orig: 'je prends B',
  tr: null,
  id: 2,
  time: '14:00:01',
  status: 'original',
  shown: null,
  lat: 0,
  err: false,
  ...over,
});

describe('feedRow', () => {
  it('formats a final voice line', () => {
    expect(feedRow(voice())).toMatchObject({ tag: 'VOIX', langs: 'RU → FR', status: 'FINAL', lat: '1,50 s', italic: false, orig: 'Бомба на A' });
  });
  it("marks a chat line already in the user's language as untranslated", () => {
    expect(feedRow(chat())).toMatchObject({ tag: 'CHAT · CT', langs: 'FR', lead: 'Lucas — ', shown: 'je prends B', orig: '', note: 'Déjà dans votre langue — non traduit' });
  });
  it('shows the original when the engine fails', () => {
    expect(feedRow(voice({ status: 'erreur', err: true, shown: null }))).toMatchObject({ shown: 'Бомба на A', orig: '', note: 'Moteur indisponible — original affiché' });
  });
});

describe('subtitle', () => {
  it('has idle copy offline and while listening', () => {
    expect(subtitle(undefined, false).text).toBe('Le silence, pour l’instant.');
    expect(subtitle(undefined, true).tag).toBe('EN ÉCOUTE');
  });
  it('quotes the latest line and flags provisional transcription', () => {
    const s = subtitle(voice({ status: 'provisoire', shown: 'Bombe…' }), true);
    expect(s).toMatchObject({ text: '« Bombe… »', italic: true, tag: 'VOIX · RUSSE → FRANÇAIS' });
    expect(s.note).toMatch(/provisoire/);
  });
});

describe('voiceLatency', () => {
  it('is null without voice lines and computes percentiles otherwise', () => {
    expect(voiceLatency([chat()])).toBeNull();
    const feed = [1000, 2000, 3000, 4000].map((lat, i) => voice({ id: i, lat }));
    expect(voiceLatency(feed)).toEqual({ p50: 3000, p95: 4000, n: 4 });
  });
});

describe('overlayBlocks', () => {
  it('uses samples before any live line and dims the third block', () => {
    const b = overlayBlocks([], 3);
    expect(b).toHaveLength(3);
    expect(b[2].opacity).toBe(0.5);
    expect(overlayBlocks([], 2).every((x) => x.opacity === 1)).toBe(true);
  });
  it('prefixes chat lines with the sender', () => {
    expect(overlayBlocks([chat({ shown: 'Je prends B' })], 3)[0]).toMatchObject({ line: 'Lucas : « Je prends B »', head: 'CHAT • ÉQUIPE CT • FRANÇAIS → FRANÇAIS' });
  });
});

describe('timecode', () => {
  it('is zero when not running and counts 25 fps frames otherwise', () => {
    expect(timecode(0, 5000, true)).toBe('00:00:00:00');
    expect(timecode(1000, 5000, false)).toBe('00:00:00:00');
    expect(timecode(0 + 1, 1 + 3_725_120, true)).toBe('01:02:05:03');
  });
});

describe('sceneTarget', () => {
  it('frames the headset per screen', () => {
    expect(sceneTarget({ entered: false, page: 'dash' }).s).toBe(0.82);
    expect(sceneTarget({ entered: true, page: 'dash' }).dim).toBe(1);
    expect(sceneTarget({ entered: true, page: 'audio' })).toMatchObject({ s: 0.32, dim: 0.5 });
  });
});

describe('latency advice', () => {
  it('points to whisper-base when the chosen model is too slow for live subtitles', async () => {
    const { latencyAdvice } = await import('./derive');
    expect(latencyAdvice(1800, 'small')).toBeNull();
    expect(latencyAdvice(6370, 'small')).toMatch(/whisper-base/);
    expect(latencyAdvice(6370, 'base')).toMatch(/processeur/);
  });
});
