// Interface language. French is the source; the other locales provide every
// key (TypeScript and i18n.test.ts check it). No dependency: plurals come
// from Intl.PluralRules (Russian, Polish… have several forms).

import { create } from 'zustand';
import { de } from './locales/de';
import { en } from './locales/en';
import { es } from './locales/es';
import { fr, type MessageKey, type Messages, type Plural } from './locales/fr';
import { pl } from './locales/pl';
import { ptBR } from './locales/pt-BR';
import { ru } from './locales/ru';
import { tr } from './locales/tr';
import { zhCN } from './locales/zh-CN';

export type { MessageKey };
export type Locale = 'fr' | 'en' | 'es' | 'de' | 'pt-BR' | 'ru' | 'pl' | 'tr' | 'zh-CN';
/** The user's choice: a locale, or the language of Windows. */
export type UiLang = Locale | 'auto';

/** In the selector: own name of each language. */
export const LOCALES: { id: Locale; name: string }[] = [
  { id: 'fr', name: 'Français' },
  { id: 'en', name: 'English' },
  { id: 'es', name: 'Español' },
  { id: 'de', name: 'Deutsch' },
  { id: 'pt-BR', name: 'Português (Brasil)' },
  { id: 'ru', name: 'Русский' },
  { id: 'pl', name: 'Polski' },
  { id: 'tr', name: 'Türkçe' },
  { id: 'zh-CN', name: '简体中文' },
];

export const MESSAGES: Record<Locale, Messages> = { fr, en, es, de, 'pt-BR': ptBR, ru, pl, tr, 'zh-CN': zhCN };

export const isLocale = (v: unknown): v is Locale => LOCALES.some((l) => l.id === v);
export const isUiLang = (v: unknown): v is UiLang => v === 'auto' || isLocale(v);

/** The best locale for the system languages (BCP-47, most preferred first);
 * English when none is supported. */
export function detectLocale(preferred: readonly string[]): Locale {
  for (const tag of preferred) {
    const t = tag.toLowerCase();
    if (t.startsWith('pt')) return 'pt-BR';
    if (t.startsWith('zh')) return 'zh-CN';
    const base = t.split('-')[0];
    const hit = LOCALES.find((l) => l.id === base);
    if (hit) return hit.id;
  }
  return 'en';
}

function systemLanguages(): readonly string[] {
  if (typeof navigator === 'undefined') return [];
  return navigator.languages?.length ? navigator.languages : navigator.language ? [navigator.language] : [];
}

export function resolveLocale(choice: UiLang, system: readonly string[] = systemLanguages()): Locale {
  return choice === 'auto' ? detectLocale(system) : choice;
}

export const useLocale = create<{ locale: Locale }>()(() => ({ locale: 'fr' }));

export type Params = Record<string, string | number>;

function fill(text: string, params?: Params): string {
  if (!params) return text;
  return text.replace(/\{(\w+)\}/g, (m, k: string) => (k in params ? String(params[k]) : m));
}

export function pick(entry: string | Plural, locale: Locale, count: number | undefined): string {
  if (typeof entry === 'string') return entry;
  if (count === undefined) return entry.other;
  const cat = new Intl.PluralRules(locale).select(count) as keyof Plural;
  return entry[cat] ?? entry.other;
}

/** Text of `key` in `locale` (French if somehow missing). A `count`
 * parameter selects the plural form. */
export function translate(locale: Locale, key: MessageKey, params?: Params): string {
  const entry = MESSAGES[locale][key] ?? fr[key];
  const count = typeof params?.count === 'number' ? params.count : undefined;
  return fill(pick(entry, locale, count), params);
}

/** Outside React (stores, helpers): the current locale. */
export const t = (key: MessageKey, params?: Params) => translate(useLocale.getState().locale, key, params);

/** In components: re-renders when the language changes. */
export function useT() {
  const locale = useLocale((s) => s.locale);
  return (key: MessageKey, params?: Params) => translate(locale, key, params);
}
