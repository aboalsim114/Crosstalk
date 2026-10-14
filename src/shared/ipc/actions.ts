// Actions from global shortcuts and the tray menu.

import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { isTauri, noop } from './core';

export type NativeAction = 'overlay' | 'pause' | 'emergency' | 'session' | 'reply' | 'dictate';


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
