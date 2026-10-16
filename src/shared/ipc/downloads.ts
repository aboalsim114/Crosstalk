// First-run download of the speech models (src-tauri/src/downloads.rs):
// The recognition model and Silero. Translation needs no download (it is done by the LLM).

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

export const modelDownload = {
  status: (model: string): Promise<DownloadStatus | null> => (isTauri ? invoke<DownloadStatus>('models_download_status', { model }) : Promise.resolve(null)),
  start: (model: string): Promise<DownloadStatus | null> => (isTauri ? invoke<DownloadStatus>('models_download_start', { model }) : Promise.resolve(null)),
  cancel: (): Promise<void> => (isTauri ? invoke<void>('models_download_cancel') : Promise.resolve()),
  onProgress: (cb: (s: DownloadStatus) => void) => (isTauri ? listen<DownloadStatus>('models://download', (e) => cb(e.payload)) : Promise.resolve(noop)),
};
