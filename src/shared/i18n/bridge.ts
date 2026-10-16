// Applies the chosen interface language: React texts, <html lang>, and the
// tray menu (labels sent to the Rust core).

import { uiLocale } from '../ipc';
import { useApp } from '../../app/state/store';
import { resolveLocale, translate, useLocale, type Locale } from './index';

/** Tray menu labels in `locale` (shortcuts after a tab, as Windows shows them). */
export function trayLabels(locale: Locale): Record<string, string> {
  const t = (k: Parameters<typeof translate>[1]) => translate(locale, k);
  return {
    show: t('tray.show'),
    session: t('tray.session'),
    overlay: `${t('action.overlay')}\tCtrl+Alt+F8`,
    reply: `${t('tray.reply')}\tAlt`,
    dictate: `${t('tray.dictate')}\tMouse 5 / Ctrl+Alt+F6`,
    pause: `${t('action.pause')}\tCtrl+Alt+F9`,
    emergency: `${t('action.emergency')}\tCtrl+Alt+F10`,
    quit: t('tray.quit'),
    tooltip: t('brand.tooltip'),
  };
}

export function startLocaleBridge(setTray: (labels: Record<string, string>) => Promise<void> = uiLocale.setTray): () => void {
  const apply = () => {
    const locale = resolveLocale(useApp.getState().uiLang);
    if (useLocale.getState().locale !== locale) useLocale.setState({ locale });
    if (typeof document !== 'undefined') document.documentElement.lang = locale;
    setTray(trayLabels(locale)).catch(() => {});
  };
  apply();
  return useApp.subscribe((s, prev) => {
    if (s.uiLang !== prev.uiLang) apply();
  });
}
