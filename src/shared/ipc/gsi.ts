// Map being played (src-tauri/src/gsi).

import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { isTauri, noop } from './core';

export interface GsiStatus {
  installed: boolean;
  cfgPath: string | null;
  cs2Found: boolean;
  listening: boolean;
  port: number;
  error: string | null;
  map: string | null;
  mapName: string | null;
  mode: string | null;
  callouts: number;
  lastSeen: number | null;
  /** Match phase (warmup, live, intermission, gameover) and score, from the same map block. */
  phase?: string | null;
  round?: number | null;
  scoreCt?: number | null;
  scoreT?: number | null;
}

export const gsi = {
  status: (): Promise<GsiStatus | null> => (isTauri ? invoke<GsiStatus>('gsi_status') : Promise.resolve(null)),
  install: (): Promise<GsiStatus> => invoke<GsiStatus>('gsi_install'),
  uninstall: (): Promise<GsiStatus> => invoke<GsiStatus>('gsi_uninstall'),
  onStatus: (cb: (s: GsiStatus) => void) => (isTauri ? listen<GsiStatus>('gsi://status', (e) => cb(e.payload)) : Promise.resolve(noop)),
};
