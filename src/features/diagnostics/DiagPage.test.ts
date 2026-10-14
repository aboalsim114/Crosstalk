import { describe, expect, it } from 'vitest';
import { journalRows, translationDiag } from './DiagPage';
import { translationHealth, translationSummary } from '../translation/translation';
import type { PairStatus, TranslationStatus } from '../../shared/ipc';

describe('diagnostic journal', () => {
  it('shows the real journal lines with their level and source', () => {
    const ts = new Date(2026, 9, 13, 14, 2, 11).getTime();
    expect(journalRows([{ ts, level: 'WARN', source: 'audio', message: 'capture impossible [processLoopback] : refusé' }])).toEqual([
      ['14:02:11', 'WARN', 'audio: capture impossible [processLoopback] : refusé'],
    ]);
  });
});

describe('translation row of the diagnostic', () => {
  const pair = (state: PairStatus['state']): PairStatus => ({ source: 'ru', target: 'fr', repo: 'Helsinki-NLP/opus-mt-ru-fr', state, error: null, loadMs: null });
  const status = (pairs: PairStatus[], enabled = true): TranslationStatus => ({ config: { enabled, glossary: [], target: 'fr', sources: ['ru'] }, pairs, modelsDir: 'models', queued: 0 });

  it('derives its verdict from the state of the models', () => {
    expect(translationDiag(translationHealth(status([pair('ready')])))).toBe('OK');
    expect(translationDiag(translationHealth(status([pair('error')])))).toBe('ÉCHEC');
    expect(translationDiag(translationHealth(status([pair('loading')])))).toBe('AVERT.');
    expect(translationDiag(translationHealth(status([pair('missing')])))).toBe('AVERT.');
    expect(translationDiag(translationHealth(status([pair('ready')], false)))).toBe('AVERT.');
    expect(translationDiag(translationHealth(null))).toBe('AVERT.');
  });

  it('keeps the colours of the summary unchanged', () => {
    expect(translationSummary(status([pair('ready')])).color).toBe('#7fe0a8');
    expect(translationSummary(status([pair('error')])).color).toBe('#ff8a7a');
    expect(translationSummary(status([pair('loading')])).color).toBe('#ffa24a');
    expect(translationSummary(status([pair('missing')])).color).toBe('#ffa24a');
    expect(translationSummary(status([pair('ready')], false)).color).toBe('#8f8a83');
    expect(translationSummary(null).color).toBe('#8f8a83');
  });
});
