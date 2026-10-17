// Languages of the translation: the user's own (everything is translated into
// it) and the players', detected in each line (src-tauri/src/translation/languages.rs).
// With an LLM every pair is direct.

/** Languages a teammate may speak or write: detected automatically, nothing to choose. */
export const PLAYER_LANGS = ['en', 'fr', 'ru', 'es', 'de', 'pt', 'uk', 'pl', 'tr', 'ar', 'zh', 'ja'] as const;
export type PlayerLang = (typeof PLAYER_LANGS)[number];

/**
 * Languages the speech recogniser writes (Parakeet TDT 0.6B v3: European
 * languages only, its vocabulary has no Arabic, Chinese, Japanese or Turkish
 * letters). The others are translated from the written chat only.
 */
export const SPEECH_LANGS = ['en', 'fr', 'ru', 'es', 'de', 'pt', 'uk', 'pl'] as const;
export type SpeechLangCode = (typeof SPEECH_LANGS)[number];
export const isSpeechLang = (v: string): v is SpeechLangCode => (SPEECH_LANGS as readonly string[]).includes(v);

/** Languages Crosstalk can translate into: all of them, with an LLM. */
export const TARGET_LANGS = PLAYER_LANGS;
export type TargetLang = (typeof TARGET_LANGS)[number];

/** Written right to left (the translated text gets dir="rtl"). */
export const RTL_LANGS: readonly string[] = ['ar'];

/** Each language in its own words, as people look for it in a list. */
export const NATIVE_NAME: Record<PlayerLang, string> = {
  en: 'English',
  fr: 'Français',
  ru: 'Русский',
  es: 'Español',
  de: 'Deutsch',
  pt: 'Português',
  uk: 'Українська',
  pl: 'Polski',
  tr: 'Türkçe',
  ar: 'العربية',
  zh: '中文',
  ja: '日本語',
};

export const isPlayerLang = (v: unknown): v is PlayerLang => typeof v === 'string' && (PLAYER_LANGS as readonly string[]).includes(v);
export const isTargetLang = (v: unknown): v is TargetLang => typeof v === 'string' && (TARGET_LANGS as readonly string[]).includes(v);

/** "en français", "en russe"… for the French texts of the intro. */
export const IN_LANGUAGE_FR: Record<TargetLang, string> = {
  en: 'en anglais',
  fr: 'en français',
  ru: 'en russe',
  es: 'en espagnol',
  de: 'en allemand',
  pt: 'en portugais',
  uk: 'en ukrainien',
  pl: 'en polonais',
  tr: 'en turc',
  ar: 'en arabe',
  zh: 'en chinois',
  ja: 'en japonais',
};

/** Before the user chooses: Windows' language when it is one of the app's, else English. */
export function guessTarget(locale: string | undefined): TargetLang {
  const l = (locale ?? '').toLowerCase().split(/[-_]/)[0];
  return isTargetLang(l) ? l : 'en';
}

/** First launch: the user's language guessed from Windows. */
export function prefillLanguages<S extends { langSetupDone: boolean; target: TargetLang }>(s: S, locale: string | undefined): Partial<S> | null {
  if (s.langSetupDone) return null;
  const target = guessTarget(locale);
  if (target === s.target) return null;
  return { target } as Partial<S>;
}

/** Languages of the `n` latest real lines (voice and chat), newest first; the feed itself is newest first. */
export function recentLangs(feed: { src: string; tid?: number }[], n = 20): string[] {
  return feed
    .filter((f) => f.tid != null)
    .slice(0, n)
    .map((f) => f.src);
}
