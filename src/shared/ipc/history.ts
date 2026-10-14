// Local history (src-tauri/src/history.rs).

import { invoke } from '@tauri-apps/api/core';
import { isTauri } from './core';

export interface HistoryEntry {
  ts: number;
  kind: 'voice' | 'chat';
  channel?: string;
  sender?: string;
  lang: string;
  original: string;
  translated?: string;
  status: string;
}

export const historyStore = {
  append: (entry: HistoryEntry): Promise<void> => (isTauri ? invoke<void>('history_append', { entry }) : Promise.resolve()),
  list: (query: string, limit: number): Promise<HistoryEntry[]> => (isTauri ? invoke<HistoryEntry[]>('history_list', { query, limit }) : Promise.resolve([])),
  count: (): Promise<number> => (isTauri ? invoke<number>('history_count') : Promise.resolve(0)),
  clear: (): Promise<void> => (isTauri ? invoke<void>('history_clear') : Promise.resolve()),
  exportTo: (format: 'txt' | 'json'): Promise<string> => (isTauri ? invoke<string>('history_export', { format }) : Promise.reject(new Error('export disponible dans l’application Windows'))),
};
