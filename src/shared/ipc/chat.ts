// Written chat of CS2 (src-tauri/src/chat).

import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { isTauri, noop } from './core';

export type ChatChannel = 'ALL' | 'CT' | 'T';
export interface ChatMessage {
  id: number;
  channel: ChatChannel;
  sender: string;
  message: string;
  location: string | null;
  dead: boolean;
  /** ru, en, fr, another code, or null when the text gives no clue. */
  lang: string | null;
}
export interface Cs2Setup {
  steamDir: string | null;
  libraries: string[];
  cs2Dir: string | null;
  consoleLog: string | null;
  consoleLogExists: boolean;
  condebug: 'present' | 'absent' | 'unknown';
  launchOptions: string | null;
  manual: boolean;
}
export interface ChatConfig {
  manualPath: string | null;
  ownName: string | null;
  ignoreOwn: boolean;
}
export type ChatState = 'idle' | 'waiting' | 'following' | 'notfound';
export interface ChatStatus {
  state: ChatState;
  setup: Cs2Setup;
  config: ChatConfig;
  rulesVersion: string;
  rulesFile: string | null;
  rulesWarning: string | null;
  offset: number;
  messages: number;
  ownIgnored: number;
  duplicates: number;
  /** Lines that look like chat but match no rule. */
  unrecognized: number;
}

export const chat = {
  status: (): Promise<ChatStatus | null> => (isTauri ? invoke<ChatStatus>('chat_status') : Promise.resolve(null)),
  configure: (config: ChatConfig): Promise<ChatStatus | null> => (isTauri ? invoke<ChatStatus>('chat_configure', { config }) : Promise.resolve(null)),
  rescan: (): Promise<ChatStatus | null> => (isTauri ? invoke<ChatStatus>('chat_rescan') : Promise.resolve(null)),
  start: (): Promise<ChatStatus | null> => (isTauri ? invoke<ChatStatus>('chat_start') : Promise.resolve(null)),
  stop: (): Promise<void> => (isTauri ? invoke<void>('chat_stop') : Promise.resolve()),
  onStatus: (cb: (s: ChatStatus) => void) => (isTauri ? listen<ChatStatus>('chat://status', (e) => cb(e.payload)) : Promise.resolve(noop)),
  onMessage: (cb: (m: ChatMessage) => void) => (isTauri ? listen<ChatMessage>('chat://message', (e) => cb(e.payload)) : Promise.resolve(noop)),
};
