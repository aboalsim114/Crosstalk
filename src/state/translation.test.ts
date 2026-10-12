import { beforeEach, describe, expect, it } from 'vitest';
import type { PairStatus, Translation, TranslationStatus } from '../backend';
import { feedRow, isDemoLine, subtitle } from '../pages/derive';
import { overlayBlocks } from '../pages/OverlayPage';
import { GLOSSARY } from './data';
import { transcriptEntry } from './speech';
import { useApp, withTranslation } from './store';
import { pairSummary, percentile, recordTime, translationConfigFrom, translationSummary, useTranslation } from './translation';

const transcript = { id: 9, text: 'Двое идут на B', lang: 'ru' as const, langProb: 0.97, audioMs: 1800, latencyMs: 1200, asrMs: 600, avgLogprob: -0.3, noSpeechProb: 0.02, model: 'base' };
const result = (over: Partial<Translation> = {}): Translation => ({
  id: 9, lang: 'ru', original: 'Двое идут на B', translated: 'Deux vont sur B', error: null, ms: 320, queueMs: 40, cached: false, glossary: true, ...over,
});
const pair = (over: Partial<PairStatus> = {}): PairStatus => ({ source: 'ru', repo: 'Helsinki-NLP/opus-mt-ru-fr', state: 'ready', error: null, loadMs: 1400, ...over });
const status = (pairs: PairStatus[], enabled = true): TranslationStatus => ({ config: { enabled, glossary: [] }, pairs, modelsDir: 'C:\\m', queued: 0 });

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
  it('keeps TRANSCRIT with the reason when the model is missing or translation is off', () => {
    const line = withTranslation(transcriptEntry(transcript), result({ translated: null, error: 'modèle de traduction absent' }));
    expect(line.status).toBe('transcrit');
    expect(line.tr).toBeNull();
    expect(feedRow(line).note).toBe('Non traduit — modèle de traduction absent');
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
  it('describes the model pairs', () => {
    expect(pairSummary(pair())).toMatchObject({ label: 'Russe → français', value: 'prêt · chargé en 1400 ms', color: '#7fe0a8' });
    expect(pairSummary(pair({ state: 'missing' })).value).toBe('modèle absent');
    expect(pairSummary(pair({ state: 'error', error: 'pytorch_model.bin illisible' })).value).toBe('pytorch_model.bin illisible');
    expect(translationSummary(null).value).toBe('…');
    expect(translationSummary(status([pair(), pair({ source: 'en', state: 'missing' })])).value).toBe('locale · RU → FR');
    expect(translationSummary(status([pair({ state: 'missing' })])).value).toBe('modèles absents');
    expect(translationSummary(status([pair()], false)).value).toBe('désactivée');
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
