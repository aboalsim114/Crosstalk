// Bridge between the React UI and the Rust core (src-tauri).
//
// The UI always runs inside the Tauri desktop app in production. These helpers also
// degrade to harmless fallbacks when the frontend is loaded outside Tauri (unit tests,
// `npm run dev` for quick styling work), so components never have to check themselves.

import { invoke, isTauri as tauriPresent } from '@tauri-apps/api/core';
import { emitTo, listen, type UnlistenFn } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';

export const isTauri: boolean = tauriPresent();

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

// ---------- actions from global shortcuts and the tray menu ----------

export type NativeAction = 'overlay' | 'pause' | 'emergency' | 'session';

const noop: UnlistenFn = () => {};

export async function onNativeAction(cb: (a: NativeAction) => void): Promise<UnlistenFn> {
  if (!isTauri) return noop;
  return listen<NativeAction>('crosstalk://action', (e) => cb(e.payload));
}

export interface ShortcutStatus {
  keys: string;
  registered: boolean;
  error: string | null;
}

export async function getShortcutStatus(): Promise<ShortcutStatus[] | null> {
  if (!isTauri) return null;
  try {
    return await invoke<ShortcutStatus[]>('shortcut_status');
  } catch {
    return null;
  }
}

// ---------- in-game overlay window ----------

export const OVERLAY_STATE_EVENT = 'overlay://state';
export const OVERLAY_READY_EVENT = 'overlay://ready';

export interface OverlayWindowConfig {
  visible: boolean;
  clickThrough: boolean;
  corner: 'tl' | 'tr' | 'bl' | 'br' | null;
}

export const overlayWindow = {
  configure: (config: OverlayWindowConfig) => (isTauri ? invoke<void>('overlay_configure', { config }) : Promise.resolve()),
  /** Main window → overlay window: what to draw. */
  pushState: <T>(state: T) => (isTauri ? emitTo('overlay', OVERLAY_STATE_EVENT, state) : Promise.resolve()),
  onState: <T>(cb: (s: T) => void) => (isTauri ? listen<T>(OVERLAY_STATE_EVENT, (e) => cb(e.payload)) : Promise.resolve(noop)),
  /** Overlay window → main window: request the current state after loading. */
  announceReady: () => (isTauri ? emitTo('main', OVERLAY_READY_EVENT) : Promise.resolve()),
  onReady: (cb: () => void) => (isTauri ? listen(OVERLAY_READY_EVENT, cb) : Promise.resolve(noop)),
};

// ---------- launch with Windows ----------

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
