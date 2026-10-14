import { act, fireEvent, render, screen } from '@testing-library/react';
import { beforeEach, describe, expect, it } from 'vitest';
import { useTargetLang } from '../i18n';
import { feedRow } from '../pages/derive';
import { LanguagePicker, LanguageSetup } from '../pages/LanguagesCard';
import { chatEntry } from './chat';
import { guessTarget, translatedSources, viaEnglish } from './langs';
import { restore, snapshot } from './persist';
import { setupNeeds } from './setup';
import { speechConfigFrom } from './speech';
import { useApp } from './store';
import { translationConfigFrom } from './translation';

const initial = useApp.getState();

beforeEach(() => {
  useApp.setState({ ...initial, target: 'fr', src: { fr: false, en: true, ru: true, de: false, es: false, uk: false, pl: false, tr: false }, langSetupDone: false });
});

describe('languages of the translation', () => {
  it('routes through English only where no direct model exists', () => {
    expect(viaEnglish('ru', 'fr')).toBe(false);
    expect(viaEnglish('de', 'ru')).toBe(true);
    expect(viaEnglish('pl', 'ru')).toBe(true);
    expect(viaEnglish('fr', 'fr')).toBe(false);
  });

  it("never translates the user's own language", () => {
    expect(translatedSources('ru', { ru: true, en: true, de: true })).toEqual(['en', 'ru', 'de'].filter((l) => l !== 'ru'));
  });

  it("guesses the user's language from Windows, English when it cannot be a target", () => {
    expect(guessTarget('ru-RU')).toBe('ru');
    expect(guessTarget('de')).toBe('de');
    expect(guessTarget('pl-PL')).toBe('en');
    expect(guessTarget(undefined)).toBe('en');
  });

  it('reaches the Rust core: translation, Whisper and the downloads', () => {
    const s = { ...useApp.getState(), target: 'ru' as const, src: { ...useApp.getState().src, en: true, de: true, ru: false } };
    expect(translationConfigFrom(s)).toMatchObject({ target: 'ru', sources: ['en', 'de'] });
    expect(speechConfigFrom(s).languages).toEqual(['en', 'de', 'ru']);
    expect(setupNeeds(s).plan).toEqual({ target: 'ru', sources: ['en', 'de'] });
  });

  it('keep the built-in French renderings for French only', () => {
    const s = useApp.getState();
    const fr = translationConfigFrom({ ...s, target: 'fr' }).glossary.find((g) => g.source === 'one low');
    expect(fr?.target).toBe('un ennemi à peu de PV');
    const ru = translationConfigFrom({ ...s, target: 'ru', gloss: [...s.gloss, ['peek', 'Traduire', 'выглянуть']] }).glossary;
    expect(ru.find((g) => g.source === 'one low')?.target).toBeNull();
    // A rendering the user wrote stays imposed.
    expect(ru.find((g) => g.source === 'peek')?.target).toBe('выглянуть');
  });

  it('are kept between launches', () => {
    const s = { ...useApp.getState(), target: 'uk' as const, langSetupDone: true };
    const back = restore(snapshot(s, null).config, initial);
    expect(back.app.target).toBe('uk');
    expect(back.app.langSetupDone).toBe(true);
    expect(restore({ version: 1, settings: { target: 'pl' } }, initial).app.target).toBeUndefined();
  });

  it('label the lines with the language of the translations', () => {
    act(() => useTargetLang.setState({ target: 'de' }));
    const line = { id: 1, tid: 1, kind: 'voice' as const, src: 'ru' as const, orig: 'Двое на B', tr: 'Zwei auf B', shown: 'Zwei auf B', status: 'final' as const, time: '12:00:00', lat: 900, err: false };
    expect(feedRow(line).langs).toBe('RU → DE');
    // A chat message already in the user's language stays as is.
    const de = chatEntry({ id: 3, channel: 'ALL', sender: 'Max', message: 'gut gemacht', lang: 'de', dead: false, time: '12:00:00' } as never, new Date(), 'de');
    expect(de.status).toBe('original');
    act(() => useTargetLang.setState({ target: 'fr' }));
  });
});

describe('language picker', () => {
  it("disables the user's own language among the players' and shows the routes", () => {
    render(<LanguagePicker />);
    fireEvent.click(screen.getByRole('radio', { name: 'Русский' }));
    expect(useApp.getState().target).toBe('ru');
    expect(screen.getByRole('checkbox', { name: 'Русский' }).getAttribute('aria-disabled')).toBe('true');
    fireEvent.click(screen.getByRole('checkbox', { name: 'Deutsch' }));
    const routes = screen.getByTestId('language-routes').textContent ?? '';
    expect(routes).toContain('EN → RU');
    expect(routes).toContain('DE → RU');
    expect(routes).toMatch(/DE → RU · via l’anglais/);
  });

  it('warns when nothing is left to translate', () => {
    useApp.setState({ src: { ...useApp.getState().src, en: false, ru: false } });
    render(<LanguagePicker />);
    expect(screen.getByTestId('language-routes').textContent).toContain('Aucune langue à traduire');
  });
});

describe('first launch', () => {
  it('asks the languages once, before anything else', () => {
    const { rerender } = render(<LanguageSetup />);
    expect(screen.getByRole('dialog')).toBeTruthy();
    fireEvent.click(screen.getByText('Continuer'));
    expect(useApp.getState().langSetupDone).toBe(true);
    rerender(<LanguageSetup />);
    expect(screen.queryByRole('dialog')).toBeNull();
  });
});
