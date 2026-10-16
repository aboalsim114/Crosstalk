// In-app update (src-tauri/src/updater.rs).

import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { isTauri, noop } from './core';

export type UpdateState = 'idle' | 'checking' | 'uptodate' | 'available' | 'downloading' | 'ready' | 'installing' | 'error';
/** One change players can notice, written for them (release-notes.json), with the build that first shipped it. */
export interface NewsEntry {
  build: number;
  date: string;
  title: string;
  items: string[];
}
export interface UpdateManifest {
  build: number;
  version: string;
  commit: string;
  date: string;
  notes: string[];
  /** What's new, newest first (absent from older feeds). */
  news?: NewsEntry[];
  setup: { url: string; sha256: string; bytes: number };
}
export interface UpdateStatus {
  state: UpdateState;
  build: number;
  version: string;
  commit: string;
  latest: UpdateManifest | null;
  doneBytes: number;
  totalBytes: number;
  error: string | null;
}

export const updater = {
  status: (): Promise<UpdateStatus | null> => (isTauri ? invoke<UpdateStatus>('update_status') : Promise.resolve(null)),
  check: (): Promise<UpdateStatus | null> => (isTauri ? invoke<UpdateStatus>('update_check') : Promise.resolve(null)),
  download: (): Promise<boolean> => (isTauri ? invoke<boolean>('update_download') : Promise.resolve(false)),
  cancel: (): Promise<void> => (isTauri ? invoke<void>('update_cancel') : Promise.resolve()),
  install: (): Promise<void> => (isTauri ? invoke<void>('update_install') : Promise.resolve()),
  onStatus: (cb: (s: UpdateStatus) => void) => (isTauri ? listen<UpdateStatus>('update://status', (e) => cb(e.payload)) : Promise.resolve(noop)),
};
