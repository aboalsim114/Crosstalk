import { beforeEach, describe, expect, it } from 'vitest';
import type { Translation, TranslationStatus } from '../../shared/ipc';
import { feedRow, isDemoLine, subtitle } from '../session/derive';
import { overlayBlocks } from '../overlay/OverlayPage';
import { GLOSSARY } from '../../app/state/data';
import { transcriptEntry } from '../speech/speech';
import { UNSURE_NOTE, useApp, visibleFeed, withTranslation } from '../../app/state/store';
import { UNSURE_LANG } from '../languages/langs';
import { percentile, recordTime, translationConfigFrom, translationHealth, translationSummary, useTranslation } from './translation';

const transcript = { id: 9, partial: false, text: 'Двое идут на B', lang: 'ru' as const, langProb: 0.97, audioMs: 1800, latencyMs: 1200, asrMs: 600, avgLogprob: -0.3, noSpeechProb: 0.02, model: 'base' };
const result = (over: Partial<Translation> = {}): Translation => ({
  id: 9, lang: 'ru', original: 'Двое идут на B', translated: 'Deux vont sur B', error: null, ms: 320, queueMs: 40, cached: false, glossary: true, ...over,
});
const status = (ready: boolean, enabled = true, problem: string | null = null): TranslationStatus => ({ config: { enabled, glossary: [], target: 'fr', sources: ['ru', 'en'] }, ready, problem, queued: 0 });

describe('a real translation turns the transcript into a French line', () => {
  const line = withTranslation(transcriptEntry(transcript), result());

  it('shows the French text with the original and the total latency', () => {
    expect(line).toMatchObject({ status: 'final', shown: 'Deux vont sur B', tr: 'Deux vont sur B', orig: 'Двое идут на B', lat: 1200 + 40 + 320, err: false });
    const row = feedRow(line);
    expect(row.langs).toBe('RU → FR');
    expect(row.shown).toBe('Deux vont sur B');
    expect(row.orig).toBe('Двое идут на B');
    expect(row.status).toBe('FINAL');
  });

  it('is never labelled as a demo, unlike the simulated chat', () => {
    expect(isDemoLine(true, line)).toBe(false);
    expect(isDemoLine(true, { ...line, tid: undefined })).toBe(true);
    expect(isDemoLine(false, { ...line, tid: undefined })).toBe(false);
  });

  it('is a French subtitle and overlay block', () => {
    expect(subtitle(line, true).tag).toBe('VOIX · RUSSE → FRANÇAIS');
    const [b] = overlayBlocks([line], 3);
    expect(b.head).toBe('VOIX • RUSSE → FRANÇAIS');
    expect(b.line).toBe('« Deux vont sur B »');
    expect(b.orig).toBe('Двое идут на B');
  });
});

describe('without a translation the original stays, never an invented French text', () => {
  it('keeps TRANSCRIT with the reason when no engine is connected or translation is off', () => {
    const line = withTranslation(transcriptEntry(transcript), result({ translated: null, error: 'aucun moteur de traduction connecté (Audio & IA › Traduction IA)' }));
    expect(line.status).toBe('transcrit');
    expect(line.tr).toBeNull();
    expect(feedRow(line).note).toBe('Non traduit — aucun moteur de traduction connecté (Audio & IA › Traduction IA)');
    expect(overlayBlocks([line], 3)[0].head).toMatch(/NON TRADUITE/);
  });

  it('marks a timeout or a full queue as an engine error, original shown', () => {
    const line = withTranslation(transcriptEntry(transcript), result({ translated: null, error: 'moteur indisponible (délai dépassé)' }));
    expect(line).toMatchObject({ status: 'erreur', err: true, shown: null, tr: null });
    const row = feedRow(line);
    expect(row.shown).toBe('Двое идут на B');
    expect(row.note).toMatch(/Moteur indisponible — original affiché \(moteur indisponible \(délai dépassé\)\)/);
  });
});

describe('store wiring', () => {
  const app0 = useApp.getState();
  beforeEach(() => useApp.setState(app0, true));

  it('attaches a translation to its own transcript only', () => {
    useApp.getState().addLine(transcriptEntry(transcript));
    useApp.getState().addLine(transcriptEntry({ ...transcript, id: 10, text: 'Бомба на A' }));
    useApp.getState().applyTranslation(result());
    const [b, a] = useApp.getState().feed;
    expect(a.status).toBe('final');
    expect(b.status).toBe('transcrit');
    // A late duplicate does not overwrite a finished line.
    useApp.getState().applyTranslation(result({ translated: 'autre' }));
    expect(useApp.getState().feed[1].shown).toBe('Deux vont sur B');
  });

  it('sends the switch and the glossary to Rust', () => {
    const c = translationConfigFrom({ tg: { ...app0.tg, translate: true }, gloss: GLOSSARY.slice(0, 3) });
    expect(c.enabled).toBe(true);
    expect(c.glossary.length).toBe(3);
    for (const [g, t] of GLOSSARY.slice(0, 3).map((g, i) => [g, c.glossary[i]] as const)) {
      expect(t.source).toBe(g[0]);
      expect(t.target).toBe(g[1] === 'Traduire' ? g[2] : null);
    }
    expect(translationConfigFrom({ tg: { ...app0.tg, translate: false }, gloss: [] }).enabled).toBe(false);
  });
});

describe('summaries', () => {
  it('describes the engine: ready with its languages, missing, off', () => {
    expect(translationSummary(null).value).toBe('…');
    expect(translationSummary(status(true))).toMatchObject({ value: 'RU, EN → FR', color: '#7fe0a8' });
    expect(translationSummary(status(false, true, 'aucun moteur de traduction connecté'))).toMatchObject({ value: 'aucun moteur connecté', color: '#ffa24a' });
    expect(translationSummary(status(true, false)).value).toBe('désactivée');
    expect(translationHealth(status(true))).toBe('ready');
    expect(translationHealth(status(false))).toBe('missing');
    expect(translationHealth(status(true, false))).toBe('off');
    expect(translationHealth(null)).toBe('off');
  });

  it('keeps real translation times only', () => {
    useTranslation.setState({ times: [] });
    recordTime(result({ ms: 300 }));
    recordTime(result({ ms: 5, cached: true }));
    recordTime(result({ translated: null, error: 'file pleine' }));
    recordTime(result({ ms: 500 }));
    expect(useTranslation.getState().times).toEqual([300, 500]);
    expect(percentile([300, 500, 400], 0.5)).toBe(400);
    expect(percentile([], 0.5)).toBeNull();
  });
});

describe('a translation made only of kept CS terms', () => {
  it('says the terms were kept, so it does not look untranslated', async () => {
    const { KEPT_NOTE } = await import('../../app/state/store');
    const line = withTranslation(transcriptEntry({ ...transcript, text: 'B, mid, smoke.' }), result({ original: 'B, mid, smoke.', translated: 'B, mid, smoke' }));
    expect(line.note).toBe(KEPT_NOTE);
    expect(feedRow(line).note).toBe(KEPT_NOTE);
    expect(withTranslation(transcriptEntry(transcript), result()).note).toBeUndefined();
  });
});

describe('lines already in the user’s language', () => {
  const fr = { ...transcript, text: 'deux en B', lang: 'fr' as const };

  it('are hidden when the language is sure, kept in the feed for the history', () => {
    const sure = transcriptEntry({ ...fr, langProb: 0.9 }, new Date(), 'fr');
    expect(sure.status).toBe('original');
    expect(visibleFeed([sure], true)).toEqual([]);
    expect(visibleFeed([sure], false)).toEqual([sure]);
    // Live words too: never flashed on screen.
    expect(transcriptEntry({ ...fr, partial: true, langProb: 0.9 }, new Date(), 'fr').status).toBe('original');
  });

  it('stay visible when the language is unsure', () => {
    const unsure = transcriptEntry({ ...fr, langProb: 0.5 }, new Date(), 'fr');
    expect(unsure.status).toBe('transcrit');
    // The LLM gave it back unchanged: shown, with why.
    const back = withTranslation(unsure, result({ lang: UNSURE_LANG, original: 'deux en B', translated: 'deux en B', target: 'fr' }));
    expect(back.status).toBe('final');
    expect(back.note).toBe(UNSURE_NOTE);
    expect(visibleFeed([back], true)).toEqual([back]);
  });

  it('are shown once in the overlay when the option is off, without "→"', () => {
    const sure = transcriptEntry({ ...fr, langProb: 0.9 }, new Date(), 'fr');
    const [b] = overlayBlocks([sure], 3);
    expect(b.orig).toBe('');
    expect(b.line).toContain('deux en B');
    expect(b.head).not.toContain('→');
  });

  it('the option is on by default and saved with the other switches', () => {
    expect(useApp.getState().tg.skipFr).toBe(true);
  });
});

describe('a translation that changed a count, a site or a side', () => {
  it('leaves the original on screen with why, not as an engine error', () => {
    const line = withTranslation(transcriptEntry(transcript), result({ translated: null, error: 'traduction écartée : nombre modifié' }));
    expect(line).toMatchObject({ status: 'transcrit', err: false, note: 'traduction écartée : nombre modifié', shown: null });
  });
});
