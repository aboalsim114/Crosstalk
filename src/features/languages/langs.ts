// Languages of the translation: the user's own (everything is translated into
// it) and the players' (src-tauri/src/translation/languages.rs). With an LLM
// every pair is direct.

/** Languages a teammate may speak or write. */
export const PLAYER_LANGS = ['fr', 'en', 'ru', 'de', 'es', 'uk', 'pl', 'tr'] as const;
export type PlayerLang = (typeof PLAYER_LANGS)[number];

/** Languages Crosstalk can translate into: all of them, with an LLM. */
export const TARGET_LANGS = PLAYER_LANGS;
export type TargetLang = (typeof TARGET_LANGS)[number];

/** Each language in its own words, as people look for it in a list. */
export const NATIVE_NAME: Record<PlayerLang, string> = {
  fr: 'Français',
  en: 'English',
  ru: 'Русский',
  de: 'Deutsch',
  es: 'Español',
  uk: 'Українська',
  pl: 'Polski',
  tr: 'Türkçe',
};

export const isPlayerLang = (v: unknown): v is PlayerLang => typeof v === 'string' && (PLAYER_LANGS as readonly string[]).includes(v);
export const isTargetLang = (v: unknown): v is TargetLang => typeof v === 'string' && (TARGET_LANGS as readonly string[]).includes(v);

/** The chosen players' languages, in the list's order. */
export function chosenSources(src: Partial<Record<PlayerLang, boolean>>): PlayerLang[] {
  return PLAYER_LANGS.filter((l) => src[l]);
}

/** Languages to translate: the players' ones except the user's own. */
export function translatedSources(target: TargetLang, src: Partial<Record<PlayerLang, boolean>>): PlayerLang[] {
  return chosenSources(src).filter((l) => l !== target);
}

/** "en français", "en russe"… for the French texts of the intro. */
export const IN_LANGUAGE_FR: Record<TargetLang, string> = { fr: 'en français', en: 'en anglais', ru: 'en russe', de: 'en allemand', es: 'en espagnol', uk: 'en ukrainien', pl: 'en polonais', tr: 'en turc' };

/** Before the user chooses: Windows' language when it is one of the app's, else English. */
export function guessTarget(locale: string | undefined): TargetLang {
  const l = (locale ?? '').toLowerCase().split(/[-_]/)[0];
  return isTargetLang(l) ? l : 'en';
}

/** First launch: the user's language guessed from Windows, the usual CS2 voice-chat languages as players'. */
export function prefillLanguages<S extends { langSetupDone: boolean; target: TargetLang; src: Record<PlayerLang, boolean> }>(s: S, locale: string | undefined): Partial<S> | null {
  if (s.langSetupDone) return null;
  const target = guessTarget(locale);
  if (target === s.target) return null;
  return { target, src: { ...s.src, [target]: false, ...(target === 'en' ? { ru: true } : { en: true }) } } as Partial<S>;
}

/** Languages of the `n` latest real lines (voice and chat), newest first; the feed itself is newest first. */
export function recentLangs(feed: { src: string; tid?: number }[], n = 20): string[] {
  return feed
    .filter((f) => f.tid != null)
    .slice(0, n)
    .map((f) => f.src);
}
