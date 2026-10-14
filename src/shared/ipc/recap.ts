// End-of-match recap (src-tauri/src/recap.rs).

import { invoke } from '@tauri-apps/api/core';

export const recapFiles = {
  /** Saves the PNG in Documents; returns the path. */
  save: async (png: Blob): Promise<string> => invoke<string>('recap_save', { png: Array.from(new Uint8Array(await png.arrayBuffer())) }),
  reveal: (path: string): Promise<void> => invoke<void>('recap_reveal', { path }),
};
