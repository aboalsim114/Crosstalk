// Mode streamer: subtitles for OBS (src-tauri/src/stream).

import { invoke } from '@tauri-apps/api/core';
import { isTauri } from './core';

export interface StreamLine {
  kind: 'voice' | 'chat';
  lang: string;
  channel?: string;
  sender?: string;
  original?: string;
  translated?: string;
  test?: boolean;
}
/** Mode streamer settings, as `stream_configure` takes them (src-tauri/src/stream). */
export interface StreamOpts {
  enabled: boolean;
  names: boolean;
  original: boolean;
  watermark: boolean;
  size: number;
  max: number;
  duration: number;
}
export interface StreamStatus {
  options: StreamOpts;
  listening: boolean;
  url: string;
  clients: number;
  sent: number;
  error: string | null;
}

export const streamOverlay = {
  status: (): Promise<StreamStatus | null> => (isTauri ? invoke<StreamStatus>('stream_status') : Promise.resolve(null)),
  configure: (options: StreamOpts): Promise<StreamStatus | null> => (isTauri ? invoke<StreamStatus>('stream_configure', { options }) : Promise.resolve(null)),
  push: (line: StreamLine): Promise<boolean> => (isTauri ? invoke<boolean>('stream_push', { line }) : Promise.resolve(false)),
};
