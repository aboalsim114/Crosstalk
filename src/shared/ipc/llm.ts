// Online translation by an LLM: the user's own provider and key, off by default
// (src-tauri/src/translation/llm.rs). The key goes to the Rust core once and
// never comes back: the status only says whether one is stored.

import { invoke } from '@tauri-apps/api/core';
import { isTauri } from './core';

/** OpenAI and every API that copies it (OpenRouter, Mistral, Groq, Ollama, LM Studio…), or Anthropic. */
export type LlmProvider = 'openai' | 'anthropic';

export interface LlmConfig {
  enabled: boolean;
  provider: LlmProvider;
  baseUrl: string;
  model: string;
}

export interface LlmStatus {
  config: LlmConfig;
  /** A key is stored (its value never leaves the Rust core). */
  hasKey: boolean;
  /** Lines are sent to the LLM. */
  active: boolean;
  /** Why it is not active although enabled. */
  problem: string | null;
  /** Where the key is kept ("Gestionnaire d'identifiants Windows"). */
  keyStorage: string;
  calls: number;
  failures: number;
  lastLatencyMs: number | null;
  lastError: string | null;
}

export const LLM_DEFAULT_URL: Record<LlmProvider, string> = {
  openai: 'https://api.openai.com/v1',
  anthropic: 'https://api.anthropic.com/v1',
};

export const llm = {
  status: (): Promise<LlmStatus | null> => (isTauri ? invoke<LlmStatus>('llm_status') : Promise.resolve(null)),
  configure: (config: LlmConfig): Promise<LlmStatus | null> => (isTauri ? invoke<LlmStatus>('llm_configure', { config }) : Promise.resolve(null)),
  /** Rejects with the reason (empty or invalid key, Credential Manager error). */
  setKey: (key: string): Promise<LlmStatus | null> => (isTauri ? invoke<LlmStatus>('llm_set_key', { key }) : Promise.resolve(null)),
  clearKey: (): Promise<LlmStatus | null> => (isTauri ? invoke<LlmStatus>('llm_clear_key') : Promise.resolve(null)),
};
