import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { SpeechStatus, Transcript } from '../../shared/ipc';
import { feedRow, subtitle } from '../session/derive';
import { overlayBlocks } from '../overlay/OverlayPage';
import { useAudio } from '../audio/audio';
import { engineSummary, formatMs, speechConfigFrom, statsLine, transcriptEntry } from './speech';
import { sceneFx, useApp } from '../../app/state/store';

const t: Transcript = {
  id: 3,
  partial: false,
  text: 'Двое идут на B',
  lang: 'ru',
  langProb: 0.97,
  audioMs: 1800,
  latencyMs: 1240,
  asrMs: 610,
  model: 'parakeet-tdt-0.6b-v3',
};

const status = (over: Partial<SpeechStatus> = {}): SpeechStatus => ({
  state: 'ready',
  config: { model: 'parakeet-tdt-0.6b-v3', languages: ['ru', 'en'], vadThreshold: 0.55 },
  modelPath: 'C:\\m\\parakeet-tdt-0.6b-v3-int8',
  vadReady: true,
  modelsDir: 'C:\\m',
  error: null,
  loadMs: 420,
  backend: 'CPU · ONNX Runtime',
  threads: 4,
  modelLabel: 'Parakeet TDT 0.6B v3',
  ...over,
});

describe('speech settings', () => {
  it('maps the UI settings to the Rust config', () => {
    const c = speechConfigFrom({ model: 'parakeet-tdt-0.6b-v3', vad: 55, src: { ru: true, en: false, de: true, es: false, fr: false, uk: false, pl: false, tr: false } });
    expect(c).toEqual({ model: 'parakeet-tdt-0.6b-v3', languages: ['ru', 'de'], vadThreshold: 0.55, voiceFocus: false });
  });

  it("listens to the players' languages and to the user's own", () => {
    const c = speechConfigFrom({ ...useApp.getState(), target: 'ru', src: { ...useApp.getState().src, ru: false, en: true, de: true } });
    expect(c.languages).toEqual(['en', 'de', 'ru']);
  });

  it('sends "Focus voix", off by default', () => {
    expect(speechConfigFrom(useApp.getState()).voiceFocus).toBe(false);
    expect(speechConfigFrom({ ...useApp.getState(), voiceFocus: true }).voiceFocus).toBe(true);
  });

  it('starts on the same model as the Rust engine, so a fresh install works', () => {
    expect(speechConfigFrom(useApp.getState()).model).toBe('parakeet-tdt-0.6b-v3');
  });

  it('never sends an empty language list or an out-of-range threshold', () => {
    const none = { ru: false, en: false, de: false, es: false, fr: false, uk: false, pl: false, tr: false };
    const c = speechConfigFrom({ model: 'parakeet-tdt-0.6b-v3', vad: 5, src: none });
    expect(c.languages).toEqual(['ru', 'en']);
    expect(c.vadThreshold).toBe(0.2);
  });
});

describe('real transcripts are never presented as French before a real translation', () => {
  const entry = transcriptEntry(t, new Date(2026, 9, 11, 14, 2, 3));

  it('becomes a voice line with no translation', () => {
    expect(entry).toMatchObject({ kind: 'voice', src: 'ru', orig: 'Двое идут на B', tr: null, shown: null, status: 'transcrit', lat: 1240, err: false, tid: 3 });
    expect(entry.time).toBe('14:02:03');
  });

  it('carries the name read on CS2\'s voice indicator, when there is one', () => {
    expect(entry.sender).toBeUndefined();
    const named = transcriptEntry({ ...t, speaker: 'Player One' });
    expect(named.sender).toBe('Player One');
    expect(feedRow(named).lead).toBe('Player One — ');
  });

  it('is labelled TRANSCRIT in the Script', () => {
    const row = feedRow(entry);
    expect(row.status).toBe('TRANSCRIT');
    expect(row.langs).toBe('RU · TRANSCRIPTION');
    expect(row.langs).not.toContain('FR');
    expect(row.shown).toBe('Двое идут на B');
    expect(row.note).toMatch(/traduction en cours/);
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
    expect(engineSummary(status())).toMatchObject({ value: 'Parakeet TDT 0.6B v3', color: '#7fe0a8' });
    expect(engineSummary(status()).detail).toBe('CPU · ONNX Runtime · 4 threads · chargé en 420 ms');
    expect(engineSummary(status({ state: 'missing' })).detail).toBe('Parakeet TDT 0.6B v3 non installé');
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

  it('simulates nothing in the desktop app: voices and chat are real', () => {
    useAudio.setState({ native: true });
    useApp.getState().toggleSession();
    vi.advanceTimersByTime(1400 + 30_000);
    expect(useApp.getState().feed).toHaveLength(0);
  });

  it('adds real lines, except in text-only mode', () => {
    useApp.getState().addLine(transcriptEntry(t));
    expect(useApp.getState().feed[0].status).toBe('transcrit');
    expect(sceneFx).toContain('ember');
    useApp.setState({ mode: 'text', feed: [] });
    useApp.getState().addLine(transcriptEntry(t));
    expect(useApp.getState().feed).toHaveLength(0);
  });

  it('shows live words while the player speaks, then the final line in place', () => {
    const { upsertTranscript } = useApp.getState();
    upsertTranscript(transcriptEntry({ ...t, partial: true, text: 'Двое' }));
    let [line] = useApp.getState().feed;
    expect(line.live).toBe(true);
    const row = feedRow(line);
    expect(row.status).toBe('EN DIRECT');
    expect(row.shown).toBe('Двое …');
    expect(row.note).toMatch(/écoute/);
    expect(subtitle(line, true).text).toBe('« Двое … »');
    expect(overlayBlocks([line], 3)[0].line).toBe('[RU] « Двое … »');

    upsertTranscript(transcriptEntry({ ...t, partial: true, text: 'Двое идут' }));
    upsertTranscript(transcriptEntry(t));
    const feed = useApp.getState().feed;
    expect(feed).toHaveLength(1);
    [line] = feed;
    expect(line.live).toBe(false);
    expect(line.orig).toBe('Двое идут на B');
    expect(feedRow(line).status).toBe('TRANSCRIT');
  });

  it('shows a provisional French translation live, kept until the careful one', () => {
    const { upsertTranscript, applyPartialTranslation, applyTranslation } = useApp.getState();
    const tr = (over: object) => ({ id: t.id, lang: 'ru', original: 'Двое', translated: 'Deux', error: null, ms: 1, queueMs: 0, cached: false, glossary: true, ...over });
    upsertTranscript(transcriptEntry({ ...t, partial: true, text: 'Двое идут' }));
    applyPartialTranslation(tr({ partial: true, translated: 'Deux vont' }));
    let [line] = useApp.getState().feed;
    let row = feedRow(line);
    expect(row.shown).toBe('Deux vont …');
    expect(row.orig).toBe('Двое идут');
    expect(row.langs).toBe('RU → FR · PROVISOIRE');
    expect(overlayBlocks([line], 3)[0].head).toBe('VOIX • RUSSE → FRANÇAIS (PROVISOIRE)');

    // Final transcript: the provisional French stays until its translation.
    upsertTranscript(transcriptEntry(t));
    [line] = useApp.getState().feed;
    expect(line.live).toBe(false);
    expect(feedRow(line).shown).toBe('Deux vont');
    expect(feedRow(line).note).toMatch(/provisoire/);
    applyTranslation(tr({ original: t.text, translated: 'Deux vont sur B' }));
    [line] = useApp.getState().feed;
    expect(line.status).toBe('final');
    expect(feedRow(line).shown).toBe('Deux vont sur B');
  });

  it('only translates the final line, and drops live lines on retraction or stop', () => {
    const { upsertTranscript, applyTranslation, retractTranscript } = useApp.getState();
    upsertTranscript(transcriptEntry({ ...t, partial: true, text: 'Двое' }));
    applyTranslation({ id: t.id, lang: 'ru', original: 'Двое', translated: 'Deux', error: null, ms: 1, queueMs: 0, cached: false, glossary: false });
    expect(useApp.getState().feed[0].status).toBe('transcrit');
    retractTranscript(t.id);
    expect(useApp.getState().feed).toHaveLength(0);

    upsertTranscript(transcriptEntry({ ...t, id: 4, partial: true, text: 'Бомба' }));
    useApp.getState().emergency();
    expect(useApp.getState().feed).toHaveLength(0);
  });
});
