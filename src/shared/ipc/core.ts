// Tauri presence, app identity and the main window's title-bar buttons.

import { invoke, isTauri as tauriPresent } from '@tauri-apps/api/core';
import type { UnlistenFn } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';

export const isTauri: boolean = tauriPresent();

/** Unsubscriber returned outside Tauri (nothing was subscribed). */
export const noop: UnlistenFn = () => {};

export interface AppInfo {
  name: string;
  version: string;
  target: string;
}

export const FALLBACK_INFO: AppInfo = { name: 'Crosstalk', version: '0.2.0', target: 'windows-x86_64' };

export async function getAppInfo(): Promise<AppInfo> {
  if (!isTauri) return FALLBACK_INFO;
  try {
    return await invoke<AppInfo>('app_info');
  } catch {
    return FALLBACK_INFO;
  }
}

/** Custom title-bar buttons (the main window is frameless to match the design). */
export const windowControls = {
  minimize: () => (isTauri ? getCurrentWindow().minimize() : Promise.resolve()),
  toggleMaximize: () => (isTauri ? getCurrentWindow().toggleMaximize() : Promise.resolve()),
  /** Hides to the notification area; the app keeps running (see src-tauri/src/lib.rs). */
  close: () => (isTauri ? getCurrentWindow().close() : Promise.resolve()),
};
