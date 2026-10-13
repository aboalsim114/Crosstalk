import { describe, expect, it } from 'vitest';
import type { SpeakerStatus, SpeakerTest } from '../backend';
import { testVerdict } from '../pages/SpeakerCard';
import { speakerConfigFrom, speakerSummary } from './speaker';
import { SPEAKER_DEFAULTS } from './store';

const st = (p: Partial<SpeakerStatus>): SpeakerStatus => ({
  enabled: true,
  state: 'watching',
  detail: null,
  ocrLang: 'fr-FR',
  known: [],
  blank: false,
  covered: false,
  ...p,
});
const test = (p: Partial<SpeakerTest>): SpeakerTest => ({
  preview: null,
  lines: [],
  names: [],
  blank: false,
  covered: false,
  foreground: true,
  error: null,
  ...p,
});

describe('"Qui parle ?"', () => {
  it("is off by default and never credits the user's own name", () => {
    expect(SPEAKER_DEFAULTS.enabled).toBe(false);
    expect(speakerConfigFrom({ speaker: SPEAKER_DEFAULTS, ownName: '  Moi ' })).toEqual({
      enabled: false,
      region: SPEAKER_DEFAULTS.region,
      ignore: ['Moi'],
    });
    expect(speakerConfigFrom({ speaker: SPEAKER_DEFAULTS, ownName: '' }).ignore).toEqual([]);
  });

  it('says why nothing is read', () => {
    expect(speakerSummary(null, false).value).toBe('Désactivé');
    expect(speakerSummary(st({ state: 'paused' }), true).value).toBe('En pause');
    expect(speakerSummary(st({ state: 'waiting', detail: "CS2 n'est pas ouvert" }), true).detail).toMatch(/CS2/);
    expect(speakerSummary(st({ blank: true }), true).detail).toMatch(/plein écran fenêtré/);
    expect(speakerSummary(st({ covered: true }), true).value).toBe('Zone recouverte');
    expect(speakerSummary(st({ state: 'error', detail: 'aucune langue' }), true)).toMatchObject({ value: 'Indisponible', detail: 'aucune langue' });
    expect(speakerSummary(st({ known: ['A', 'B'] }), true).detail).toBe('2 pseudos lus pendant cette session.');
  });

  it('explains the calibration test', () => {
    expect(testVerdict(null)).toBeNull();
    expect(testVerdict(test({ names: ['Player One'] }))?.text).toBe('Lu : Player One');
    expect(testVerdict(test({ foreground: false }))?.text).toMatch(/premier plan/);
    expect(testVerdict(test({ blank: true }))?.text).toMatch(/fenêtré/);
    expect(testVerdict(test({ lines: ['1:45'] }))?.text).toMatch(/aucun pseudo/);
    expect(testVerdict(test({}))?.text).toMatch(/ajustez la zone/);
    expect(testVerdict(test({ error: "CS2 n'est pas ouvert" }))?.color).toBe('#ff8a7a');
  });
});
