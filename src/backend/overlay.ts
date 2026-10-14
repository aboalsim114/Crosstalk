// In-game overlay window.

import { invoke } from '@tauri-apps/api/core';
import { emitTo, listen } from '@tauri-apps/api/event';
import { isTauri, noop } from './core';

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
