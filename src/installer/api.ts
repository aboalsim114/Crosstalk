// Bridge to the installer's Rust side (crosstalk-setup). In a plain browser
// (design review, tests) a simulation stands in, clearly labelled as such.

import { invoke, isTauri as tauriPresent } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';

export const native = tauriPresent();

export interface SetupInfo {
  version: string;
  defaultDir: string;
  /** Application files (embedded in the installer). */
  appBytes: number;
  /** Models still to download into %LOCALAPPDATA%\Crosstalk\models. */
  modelBytes: number;
  /** Version already installed, if any (reinstall / update). */
  installed: string | null;
}

export interface SetupProgress {
  /** 0 préparation, 1 extraction, 2 configuration, 3 finalisation. */
  phase: number;
  /** Overall progress, 0..100. */
  pct: number;
  detail: string;
  done: boolean;
  error: string | null;
  cancelled: boolean;
}

export interface InstallOptions {
  dir: string;
  desktop: boolean;
}

const SIM_INFO: SetupInfo = { version: '0.4.0', defaultDir: 'C:\\Users\\vous\\AppData\\Local\\Crosstalk', appBytes: 26_000_000, modelBytes: 776_000_000, installed: null };

let simTimer: ReturnType<typeof setInterval> | undefined;
const simListeners = new Set<(p: SetupProgress) => void>();

function simulate(): void {
  clearInterval(simTimer);
  let pct = 0;
  simTimer = setInterval(() => {
    pct = Math.min(100, pct + 0.9);
    const phase = pct < 12 ? 0 : pct < 62 ? 1 : pct < 88 ? 2 : 3;
    const done = pct >= 100;
    if (done) clearInterval(simTimer);
    simListeners.forEach((cb) => cb({ phase, pct, detail: 'Simulation (aperçu du design)', done, error: null, cancelled: false }));
  }, 50);
}

export const setup = {
  info: (): Promise<SetupInfo> => (native ? invoke<SetupInfo>('setup_info') : Promise.resolve(SIM_INFO)),
  freeSpace: (dir: string): Promise<number | null> => (native ? invoke<number | null>('setup_free_space', { dir }) : Promise.resolve(214_000_000_000)),
  pickFolder: (current: string): Promise<string | null> => (native ? invoke<string | null>('setup_pick_folder', { current }) : Promise.resolve(null)),
  install: (opts: InstallOptions): Promise<void> => (native ? invoke<void>('setup_install', { opts }) : Promise.resolve(simulate())),
  cancel: (): Promise<void> => (native ? invoke<void>('setup_cancel') : Promise.resolve(clearInterval(simTimer))),
  launch: (dir: string): Promise<void> => (native ? invoke<void>('setup_launch', { dir }) : Promise.resolve()),
  setAutostart: (dir: string, on: boolean): Promise<void> => (native ? invoke<void>('setup_autostart', { dir, on }) : Promise.resolve()),
  onProgress: (cb: (p: SetupProgress) => void): Promise<() => void> => {
    if (native) return listen<SetupProgress>('setup://progress', (e) => cb(e.payload));
    simListeners.add(cb);
    return Promise.resolve(() => simListeners.delete(cb));
  },
  minimize: () => (native ? getCurrentWindow().minimize() : Promise.resolve()),
  quit: () => (native ? invoke<void>('setup_quit') : Promise.resolve()),
};

/** 412 Mo / 1,2 Go, as the design writes sizes. */
export function size(bytes: number): string {
  if (bytes >= 1e12) return `${(bytes / 1e12).toFixed(1).replace('.', ',')} To`;
  if (bytes >= 1e9) return `${(bytes / 1e9).toFixed(bytes >= 1e10 ? 0 : 1).replace('.', ',')} Go`;
  return `${Math.max(1, Math.round(bytes / 1e6))} Mo`;
}
