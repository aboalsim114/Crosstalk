// Interface language (src-tauri/src/tray.rs).

import { invoke } from '@tauri-apps/api/core';
import { isTauri } from './core';

/** Translated tray menu labels, by item id. */
export const uiLocale = {
  setTray: (labels: Record<string, string>): Promise<void> => (isTauri ? invoke<void>('ui_set_tray_labels', { labels }) : Promise.resolve()),
};
