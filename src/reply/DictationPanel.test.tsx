import { describe, expect, it } from 'vitest';
import { render, screen } from '@testing-library/react';
import type { DictationState } from '../backend';
import { DictationPanel, dictationView } from './DictationPanel';
import { PASTE_HINT } from './ReplyBox';

const st = (over: Partial<DictationState>): DictationState => ({ state: 'listening', level: 0.5, seconds: 1.4, text: null, translated: null, target: 'ru', via: null, copied: false, error: null, ...over });

describe('spoken reply', () => {
  it('shows each step honestly', () => {
    expect(dictationView(st({})).title).toBe('Parlez en français…');
    expect(dictationView(st({})).detail).toMatch(/^1,4 s/);
    expect(dictationView(st({ state: 'transcribing' })).detail).toBe('Sur ce PC, vers le russe');
    const done = dictationView(st({ state: 'done', text: 'deux en B, je drop', translated: 'Двое на Б', via: 'model', copied: true }));
    expect(done.title).toBe('Двое на Б');
    expect(done.detail).toBe(`Traduit sur ce PC · ${PASTE_HINT}`);
    expect(dictationView(st({ state: 'done', translated: 'x', copied: false })).detail).toMatch(/Copie impossible/);
    expect(dictationView(st({ state: 'error', error: 'Aucune parole entendue' })).detail).toBe('Aucune parole entendue');
  });

  it('shows what was understood and the translation', () => {
    render(<DictationPanel st={st({ state: 'done', text: 'ils sont trois en A', translated: 'They are three A', via: 'model', copied: true, target: 'en' })} />);
    expect(screen.getByText('« ils sont trois en A »')).toBeTruthy();
    expect(screen.getByText('They are three A')).toBeTruthy();
  });
});
