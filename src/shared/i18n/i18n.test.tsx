import { beforeEach, describe, expect, it } from 'vitest';
import { render, screen } from '@testing-library/react';
import { Sidebar } from '../../app/shell/Sidebar';
import { useApp } from '../../app/state/store';
import { restore, snapshot } from '../../app/state/persist';
import { startLocaleBridge, trayLabels } from './bridge';
import { detectLocale, LOCALES, MESSAGES, pick, resolveLocale, translate, useLocale } from './index';
import { fr } from './locales/fr';

const placeholders = (s: string) => [...s.matchAll(/\{(\w+)\}/g)].map((m) => m[1]).sort();
const texts = (v: unknown): string[] => (typeof v === 'string' ? [v] : Object.values(v as Record<string, string>));

describe('interface languages', () => {
  it('every locale has every key, non-empty, with the same placeholders as French', () => {
    const keys = Object.keys(fr).sort();
    for (const { id } of LOCALES) {
      const m = MESSAGES[id];
      expect(Object.keys(m).sort(), id).toEqual(keys);
      for (const k of keys) {
        const ref = placeholders(texts(fr[k as keyof typeof fr])[0]);
        for (const text of texts(m[k as keyof typeof m])) {
          expect(text.trim(), `${id} ${k}`).not.toBe('');
          expect(placeholders(text), `${id} ${k}`).toEqual(ref);
        }
      }
    }
  });

  it('follows the language of Windows when asked', () => {
    expect(detectLocale(['ru-RU', 'en-US'])).toBe('ru');
    expect(detectLocale(['pt-PT'])).toBe('pt-BR');
    expect(detectLocale(['zh-TW', 'en'])).toBe('zh-CN');
    expect(detectLocale(['ja-JP', 'de-AT'])).toBe('de');
    expect(detectLocale(['ja-JP'])).toBe('en');
    expect(detectLocale([])).toBe('en');
    expect(resolveLocale('auto', ['fr-CA'])).toBe('fr');
    expect(resolveLocale('pl', ['fr-CA'])).toBe('pl');
  });

  it('fills placeholders and picks plural forms', () => {
    expect(translate('en', 'header.chapter', { n: '03', total: '07' })).toBe('CHAPTER 03 / 07');
    expect(translate('zh-CN', 'uiLang.auto', { name: 'English' })).toBe('自动（English）');
    const files = { one: '{count} файл', few: '{count} файла', many: '{count} файлов', other: '{count} файла' };
    expect([1, 3, 5, 21].map((n) => pick(files, 'ru', n).replace('{count}', String(n)))).toEqual(['1 файл', '3 файла', '5 файлов', '21 файл']);
    expect(pick({ one: 'plik', few: 'pliki', many: 'plików', other: 'pliku' }, 'pl', 12)).toBe('plików');
  });
});

describe('applying the language', () => {
  const app0 = useApp.getState();
  beforeEach(() => {
    useApp.setState(app0, true);
    useLocale.setState({ locale: 'fr' });
  });

  it('stays French by default until every screen is translated, and is kept between launches', () => {
    expect(app0.uiLang).toBe('fr');
    expect(restore(snapshot({ ...app0, uiLang: 'ru' }, null).config, app0).app.uiLang).toBe('ru');
    expect(restore({ version: 1, settings: { uiLang: 'xx' } }, app0).app.uiLang).toBeUndefined();
    expect(restore({ version: 1, settings: { uiLang: 'auto' } }, app0).app.uiLang).toBe('auto');
  });

  it('translates the interface and the tray menu when the language changes', async () => {
    const sent: Record<string, string>[] = [];
    const stop = startLocaleBridge(async (l) => void sent.push(l));
    expect(sent[0].quit).toBe('Quitter Crosstalk');
    render(<Sidebar />);
    expect(screen.getByText('Accueil')).toBeTruthy();
    useApp.getState().set({ uiLang: 'en' });
    expect(useLocale.getState().locale).toBe('en');
    expect(document.documentElement.lang).toBe('en');
    expect(await screen.findByText('Home')).toBeTruthy();
    expect(screen.getByText('Emergency stop')).toBeTruthy();
    expect(sent.at(-1)).toMatchObject({ quit: 'Quit Crosstalk', overlay: 'Overlay\tCtrl+Alt+F8', tooltip: 'Crosstalk — CS2 translation' });
    stop();
    expect(trayLabels('de').emergency).toBe('Notstopp\tCtrl+Alt+F10');
  });
});
