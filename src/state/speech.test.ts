import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { SpeechStatus, Transcript } from '../backend';
import { feedRow, subtitle } from '../pages/derive';
import { overlayBlocks } from '../pages/OverlayPage';
import { useAudio } from './audio';
import { GLOSSARY } from './data';
import { engineSummary, formatMs, speechConfigFrom, statsLine, transcriptEntry } from './speech';
import { sceneFx, useApp } from './store';

const t: Transcript = {
  id: 3,
  text: 'Двое идут на B',
  lang: 'ru',
  langProb: 0.97,
  audioMs: 1800,
  latencyMs: 1240,
  asrMs: 610,
  avgLogprob: -0.31,
  noSpeechProb: 0.02,
  model: 'base',
};

const status = (over: Partial<SpeechStatus> = {}): SpeechStatus => ({
  state: 'ready',
  config: { model: 'base', languages: ['ru', 'en'], vadThreshold: 0.55, glossary: [] },
  modelPath: 'C:\\m\\ggml-base.bin',
  vadReady: true,
  modelsDir: 'C:\\m',
  error: null,
  loadMs: 420,
  backend: 'CPU · AVX2',
  threads: 4,
  ...over,
});

describe('speech settings', () => {
  it('maps the UI settings to the Rust config', () => {
    const c = speechConfigFrom({ model: 'small', vad: 55, src: { ru: true, en: false, de: true, es: false, pt: false, pl: false }, gloss: GLOSSARY.slice(0, 2) });
    expect(c).toEqual({ model: 'small', languages: ['ru'], vadThreshold: 0.55, glossary: ['rush B', 'mid'] });
  });

  it('starts on the same model as the Rust engine (base), so a fresh install works', () => {
    expect(speechConfigFrom(useApp.getState()).model).toBe('base');
  });

  it('never sends an empty language list or an out-of-range threshold', () => {
    const c = speechConfigFrom({ model: 'base', vad: 5, src: { ru: false, en: false, de: true, es: false, pt: false, pl: false }, gloss: [] });
    expect(c.languages).toEqual(['ru', 'en']);
    expect(c.vadThreshold).toBe(0.2);
  });
});

describe('real transcripts are never presented as French translations', () => {
  const entry = transcriptEntry(t, new Date(2026, 9, 11, 14, 2, 3));

  it('becomes a voice line with no translation', () => {
    expect(entry).toMatchObject({ kind: 'voice', src: 'ru', orig: 'Двое идут на B', tr: null, shown: null, status: 'transcrit', lat: 1240, err: false });
    expect(entry.time).toBe('14:02:03');
  });

  it('is labelled TRANSCRIT in the Script', () => {
    const row = feedRow(entry);
    expect(row.status).toBe('TRANSCRIT');
    expect(row.langs).toBe('RU · TRANSCRIPTION');
    expect(row.langs).not.toContain('FR');
    expect(row.shown).toBe('Двое идут на B');
    expect(row.note).toMatch(/traduction en français à venir/);
  });

  it('says "transcription" in the big subtitle', () => {
    const sub = subtitle(entry, true);
    expect(sub.tag).toBe('VOIX · RUSSE · TRANSCRIPTION');
    expect(sub.tag).not.toContain('FRANÇAIS');
    expect(sub.text).toBe('« Двое идут на B »');
    expect(sub.note).toMatch(/langue originale/);
  });

  it('is marked as untranslated in the overlay, even in minimal format', () => {
    const [b] = overlayBlocks([entry], 3);
    expect(b.head).toBe('VOIX • RUSSE • TRANSCRIPTION (NON TRADUITE)');
    expect(b.line).toBe('[RU] « Двое идут на B »');
    expect(b.orig).toBe('');
  });
});

describe('engine and stats summaries', () => {
  it('describes each engine state', () => {
    expect(engineSummary(null).value).toBe('…');
    expect(engineSummary(status())).toMatchObject({ value: 'Whisper base', color: '#7fe0a8' });
    expect(engineSummary(status()).detail).toBe('CPU · AVX2 · 4 threads · chargé en 420 ms');
    expect(engineSummary(status({ state: 'loading' })).value).toBe('Chargement…');
    expect(engineSummary(status({ state: 'missing' })).value).toBe('Modèle absent');
    expect(engineSummary(status({ state: 'missing', vadReady: false })).detail).toMatch(/silero/);
    expect(engineSummary(status({ state: 'error', error: 'modèle illisible' }))).toMatchObject({ value: 'Erreur', detail: 'modèle illisible' });
  });

  it('formats durations and counters', () => {
    expect(formatMs(640)).toBe('640 ms');
    expect(formatMs(1840)).toBe('1,8 s');
    expect(statsLine(null)).toMatch(/Aucun segment/);
    expect(
      statsLine({ segments: 9, transcribed: 4, rejected: { transient: 3, hallucination: 1, short: 1 }, droppedBlocks: 0, latencyP50Ms: 900, latencyP95Ms: 1500, lastRtf: 0.3, cpuPercent: 12 }),
    ).toBe('4 transcrits · 5 rejetés (tir / explosion 3, hallucination 1, trop court 1)');
  });
});

describe('store with real voices', () => {
  const app0 = useApp.getState();
  const audio0 = useAudio.getState();
  beforeEach(() => {
    vi.useFakeTimers();
    useApp.setState(app0, true);
    sceneFx.length = 0;
  });
  afterEach(() => {
    useApp.getState().emergency();
    useAudio.setState(audio0, true);
    vi.useRealTimers();
  });

  it('no longer simulates voices in the desktop app (chat only)', () => {
    useAudio.setState({ native: true });
    useApp.getState().toggleSession();
    vi.advanceTimersByTime(1400 + 30_000);
    const feed = useApp.getState().feed;
    expect(feed.length).toBeGreaterThan(0);
    expect(feed.every((f) => f.kind === 'chat')).toBe(true);
  });

  it('adds real lines, except in text-only mode', () => {
    useApp.getState().addLine(transcriptEntry(t));
    expect(useApp.getState().feed[0].status).toBe('transcrit');
    expect(sceneFx).toContain('ember');
    useApp.setState({ mode: 'text', feed: [] });
    useApp.getState().addLine(transcriptEntry(t));
    expect(useApp.getState().feed).toHaveLength(0);
  });
});
