// Bridge between the React UI and the Rust core (src-tauri).
//
// The UI always runs inside the Tauri desktop app in production. These helpers also
// degrade to harmless fallbacks when the frontend is loaded outside Tauri (unit tests,
// `npm run dev` for quick styling work), so components never have to check themselves.

import { invoke, isTauri as tauriPresent } from '@tauri-apps/api/core';

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
