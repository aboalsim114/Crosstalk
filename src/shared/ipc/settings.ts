// Settings on disk (src-tauri/src/settings.rs).

import { invoke } from '@tauri-apps/api/core';
import { isTauri } from './core';

export interface StoredSettings {
  config: unknown;
  glossary: unknown;
  warnings: string[];
  dir: string;
}

export const settingsStore = {
  load: (): Promise<StoredSettings | null> => (isTauri ? invoke<StoredSettings>('settings_load') : Promise.resolve(null)),
  save: (config: unknown, glossary: unknown): Promise<void> => (isTauri ? invoke<void>('settings_save', { config, glossary }) : Promise.resolve()),
};
