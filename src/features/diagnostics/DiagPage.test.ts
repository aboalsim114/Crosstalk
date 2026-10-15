import { describe, expect, it } from 'vitest';
import { journalRows, translationDiag } from './DiagPage';
import { translationHealth, translationSummary } from '../translation/translation';
import type { TranslationStatus } from '../../shared/ipc';

describe('diagnostic journal', () => {
  it('shows the real journal lines with their level and source', () => {
    const ts = new Date(2026, 9, 13, 14, 2, 11).getTime();
    expect(journalRows([{ ts, level: 'WARN', source: 'audio', message: 'capture impossible [processLoopback] : refusé' }])).toEqual([
      ['14:02:11', 'WARN', 'audio: capture impossible [processLoopback] : refusé'],
    ]);
  });
});

describe('translation row of the diagnostic', () => {
  const status = (ready: boolean, enabled = true): TranslationStatus => ({ config: { enabled, glossary: [], target: 'fr', sources: ['ru'] }, ready, problem: ready ? null : 'aucun moteur de traduction connecté', queued: 0 });

  it('derives its verdict from the state of the engine', () => {
    expect(translationDiag(translationHealth(status(true)))).toBe('OK');
    expect(translationDiag(translationHealth(status(false)))).toBe('AVERT.');
    expect(translationDiag(translationHealth(status(true, false)))).toBe('AVERT.');
    expect(translationDiag(translationHealth(null))).toBe('AVERT.');
  });

  it('keeps the colours of the summary unchanged', () => {
    expect(translationSummary(status(true)).color).toBe('#7fe0a8');
    expect(translationSummary(status(false)).color).toBe('#ffa24a');
    expect(translationSummary(status(true, false)).color).toBe('#8f8a83');
    expect(translationSummary(null).color).toBe('#8f8a83');
  });
});
