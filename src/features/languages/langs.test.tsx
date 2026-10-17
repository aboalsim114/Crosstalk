import { act, fireEvent, render, screen } from '@testing-library/react';
import { beforeEach, describe, expect, it } from 'vitest';
import { useTargetLang } from '../../shared/i18n';
import { feedRow } from '../session/derive';
import { LanguagePicker, LanguageSetup, MyLanguagesCard } from './LanguagesCard';
import { chatEntry } from '../chat/chat';
import { guessTarget, isSpeechLang, PLAYER_LANGS, SPEECH_LANGS, TARGET_LANGS } from './langs';
import { restore, snapshot } from '../../app/state/persist';
import { setupNeeds } from '../setup/setup';
import { speechConfigFrom } from '../speech/speech';
import { useApp } from '../../app/state/store';
import { translationConfigFrom } from '../translation/translation';

const initial = useApp.getState();

beforeEach(() => {
  useApp.setState({ ...initial, target: 'fr', langSetupDone: false });
});

describe('languages of the translation', () => {
  it('translates into every language directly, with the LLM', () => {
    expect([...TARGET_LANGS]).toEqual([...PLAYER_LANGS]);
  });

  it('offers the 10 languages asked for, plus Ukrainian and Polish', () => {
    for (const l of ['en', 'fr', 'ar', 'ru', 'zh', 'es', 'de', 'pt', 'tr', 'ja', 'uk', 'pl']) expect(TARGET_LANGS).toContain(l);
  });

  it('never claims voice recognition the model does not have', () => {
    // Parakeet TDT 0.6B v3: European languages only, no Arabic, Chinese, Japanese or Turkish letters.
    expect([...SPEECH_LANGS].sort()).toEqual(['de', 'en', 'es', 'fr', 'pl', 'pt', 'ru', 'uk']);
    for (const l of ['ar', 'zh', 'ja', 'tr']) expect(isSpeechLang(l)).toBe(false);
  });

  it("guesses the user's language from Windows, English when it cannot be a target", () => {
    expect(guessTarget('ru-RU')).toBe('ru');
    expect(guessTarget('de')).toBe('de');
    expect(guessTarget('pl-PL')).toBe('pl');
    expect(guessTarget('it-IT')).toBe('en');
    expect(guessTarget(undefined)).toBe('en');
  });

  it('reaches the Rust core with every language detected, nothing to choose', () => {
    const s = { ...useApp.getState(), target: 'ru' as const };
    expect(translationConfigFrom(s)).toMatchObject({ target: 'ru', sources: [...PLAYER_LANGS] });
    expect(speechConfigFrom(s).languages).toEqual([...SPEECH_LANGS]);
    expect(setupNeeds(s)).toEqual({ model: s.model });
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
    expect(restore({ version: 1, settings: { target: 'pl' } }, initial).app.target).toBe('pl');
    expect(restore({ version: 1, settings: { target: 'it' } }, initial).app.target).toBeUndefined();
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
  it('chooses the target only; the players’ languages are listed, voice or chat only', () => {
    render(<LanguagePicker />);
    fireEvent.click(screen.getByRole('radio', { name: 'العربية' }));
    expect(useApp.getState().target).toBe('ar');
    expect(screen.queryAllByRole('checkbox')).toHaveLength(0);
    const detected = screen.getByTestId('detected-languages').textContent ?? '';
    expect(detected).toMatch(/Русский.*voix et chat/);
    expect(detected).toMatch(/Türkçe.*中文.*日本語.*chat écrit seulement/);
    // The user's own language is not listed among the players'.
    expect(detected).not.toContain('العربية');
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

describe('hiding the lines already in my language', () => {
  it('is a switch of Mes langues, on by default, and saved', () => {
    useApp.setState({ tg: { ...useApp.getState().tg, skipFr: true } });
    render(<MyLanguagesCard />);
    const sw = screen.getByRole('switch', { name: 'Masquer les phrases déjà dans ma langue' });
    expect(sw.getAttribute('aria-checked')).toBe('true');
    fireEvent.click(sw);
    expect(useApp.getState().tg.skipFr).toBe(false);
    expect((snapshot(useApp.getState(), null).config.settings as { tg?: object }).tg).toMatchObject({ skipFr: false });
  });
});
