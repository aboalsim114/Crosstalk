// First-run model download (src-tauri/src/downloads.rs).

import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { isTauri, noop } from './core';

export type DownloadState = 'idle' | 'downloading' | 'done' | 'error' | 'cancelled';
export interface DownloadStatus {
  state: DownloadState;
  current: string | null;
  doneBytes: number;
  totalBytes: number;
  missingFiles: number;
  missingBytes: number;
  error: string | null;
}

/** The user's language and the players', for the translation models to fetch. */
export interface TranslationPlan {
  target: string;
  sources: string[];
}

export const modelDownload = {
  status: (model: string, translation: boolean, plan?: TranslationPlan): Promise<DownloadStatus | null> =>
    isTauri ? invoke<DownloadStatus>('models_download_status', { model, translation, plan: plan ?? null }) : Promise.resolve(null),
  start: (model: string, translation: boolean, plan?: TranslationPlan): Promise<DownloadStatus | null> =>
    isTauri ? invoke<DownloadStatus>('models_download_start', { model, translation, plan: plan ?? null }) : Promise.resolve(null),
  cancel: (): Promise<void> => (isTauri ? invoke<void>('models_download_cancel') : Promise.resolve()),
  onProgress: (cb: (s: DownloadStatus) => void) => (isTauri ? listen<DownloadStatus>('models://download', (e) => cb(e.payload)) : Promise.resolve(noop)),
};
