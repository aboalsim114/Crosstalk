// Wires the store to the native side: global shortcuts / tray actions in, and the
// in-game overlay window out. Only active inside the Tauri app.

import { isTauri, onNativeAction, overlayWindow, type NativeAction, type OverlayWindowConfig } from './backend';
import { isRunning, useApp, type AppState, type FeedEntry } from './state/store';

export interface OverlayPayload {
  feed: FeedEntry[];
  maxBlocks: number;
  fmt: AppState['fmt'];
  opacity: number;
  textSize: number;
  /** Seconds before a line fades out. */
  duration: number;
  corner: AppState['corner'];
  /** Repositioning: show placeholder lines and a dashed frame, accept drags. */
  repos: boolean;
}

export function overlayPayload(s: AppState): OverlayPayload {
  return {
    feed: s.feed.slice(0, 3),
    maxBlocks: s.maxBlocks,
    fmt: s.fmt,
    opacity: s.opacity,
    textSize: s.textSize,
    duration: s.duration,
    corner: s.corner,
    repos: s.repos,
  };
}

/** Window flags derived from the settings: shown while a session runs (or when repositioning). */
export function overlayConfig(s: AppState, prev?: AppState): OverlayWindowConfig {
  return {
    visible: s.overlayOn && (isRunning(s) || s.repos),
    clickThrough: s.tg.click && !s.repos,
    corner: !prev || prev.corner !== s.corner ? s.corner : null,
  };
}

export function applyNativeAction(a: NativeAction) {
  const s = useApp.getState();
  ({ overlay: s.toggleOverlay, pause: s.togglePause, emergency: s.emergency, session: s.toggleSession })[a]();
}

export function startNativeBridge(): () => void {
  if (!isTauri) return () => {};
  const unlisten: Promise<() => void>[] = [];
  unlisten.push(onNativeAction(applyNativeAction));

  const push = () => overlayWindow.pushState(overlayPayload(useApp.getState()));
  unlisten.push(overlayWindow.onReady(push));

  let lastCfg = '';
  const sync = (s: AppState, prev?: AppState) => {
    const cfg = overlayConfig(s, prev);
    const key = JSON.stringify(cfg);
    if (key !== lastCfg) {
      lastCfg = key;
      overlayWindow.configure(cfg).catch((e) => console.warn('overlay_configure failed', e));
    }
    if (!prev || prev.feed !== s.feed || prev.fmt !== s.fmt || prev.opacity !== s.opacity || prev.textSize !== s.textSize || prev.maxBlocks !== s.maxBlocks || prev.duration !== s.duration || prev.corner !== s.corner || prev.repos !== s.repos) push();
  };
  sync(useApp.getState());
  const unsub = useApp.subscribe(sync);

  return () => {
    unsub();
    unlisten.forEach((p) => p.then((f) => f()));
  };
}
