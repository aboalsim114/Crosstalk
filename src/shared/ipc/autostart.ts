// Launch with Windows.

import { isTauri } from './core';

export const autostart = {
  isEnabled: async (): Promise<boolean> => {
    if (!isTauri) return false;
    const { isEnabled } = await import('@tauri-apps/plugin-autostart');
    return isEnabled();
  },
  set: async (on: boolean): Promise<void> => {
    if (!isTauri) return;
    const { enable, disable } = await import('@tauri-apps/plugin-autostart');
    await (on ? enable() : disable());
  },
};
