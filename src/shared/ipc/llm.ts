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

/** Result of "Tester la connexion". */
export interface LlmTest {
  ok: boolean;
  ms: number;
  original: string;
  translated: string | null;
  error: string | null;
}

/** Ollama running on this PC and its installed models. */
export interface OllamaLocal {
  running: boolean;
  models: string[];
}

/** The services offered in Audio & IA: address and suggested model filled in, the user only adds a key. */
export type LlmService = 'openai' | 'openrouter' | 'mistral' | 'groq' | 'anthropic' | 'ollama-cloud' | 'ollama' | 'lmstudio' | 'custom';

export interface LlmServiceInfo {
  id: LlmService;
  name: string;
  provider: LlmProvider;
  baseUrl: string;
  /** Suggested model, editable in the advanced settings. */
  model: string;
  /** Runs on this PC: no key, nothing leaves the network. */
  local: boolean;
  /** Short line under the name. */
  note: string;
}

// Model names are suggestions: each service changes its catalogue, the user can type another one.
export const LLM_SERVICES: LlmServiceInfo[] = [
  { id: 'openai', name: 'OpenAI', provider: 'openai', baseUrl: 'https://api.openai.com/v1', model: 'gpt-4o-mini', local: false, note: 'Clé payante (compte avec moyen de paiement)' },
  { id: 'openrouter', name: 'OpenRouter', provider: 'openai', baseUrl: 'https://openrouter.ai/api/v1', model: 'openai/gpt-4o-mini', local: false, note: 'Un seul compte, de nombreux modèles' },
  { id: 'mistral', name: 'Mistral AI', provider: 'openai', baseUrl: 'https://api.mistral.ai/v1', model: 'mistral-small-latest', local: false, note: 'Service français' },
  { id: 'groq', name: 'Groq', provider: 'openai', baseUrl: 'https://api.groq.com/openai/v1', model: 'llama-3.1-8b-instant', local: false, note: 'Réponses très rapides' },
  { id: 'anthropic', name: 'Anthropic (Claude)', provider: 'anthropic', baseUrl: 'https://api.anthropic.com/v1', model: 'claude-haiku-5-5', local: false, note: 'Modèles Claude' },
  { id: 'ollama-cloud', name: 'Ollama Cloud', provider: 'openai', baseUrl: 'https://ollama.com/v1', model: 'gemma4:31b', local: false, note: 'Modèles ouverts hébergés par Ollama' },
  { id: 'ollama', name: 'Ollama sur ce PC', provider: 'openai', baseUrl: 'http://localhost:11434/v1', model: '', local: true, note: 'Gratuit, sans clé, rien ne quitte le PC' },
  { id: 'lmstudio', name: 'LM Studio sur ce PC', provider: 'openai', baseUrl: 'http://localhost:1234/v1', model: '', local: true, note: 'Gratuit, sans clé, rien ne quitte le PC' },
];

export const llmService = (id: LlmService): LlmServiceInfo | undefined => LLM_SERVICES.find((s) => s.id === id);

/** The service a saved configuration belongs to (settings written before the service list). */
export function serviceOf(c: Pick<LlmConfig, 'provider' | 'baseUrl'>): LlmService {
  const url = c.baseUrl.trim().replace(/\/+$/, '');
  return LLM_SERVICES.find((s) => s.provider === c.provider && s.baseUrl === url)?.id ?? 'custom';
}

export const llm = {
  status: (): Promise<LlmStatus | null> => (isTauri ? invoke<LlmStatus>('llm_status') : Promise.resolve(null)),
  configure: (config: LlmConfig): Promise<LlmStatus | null> => (isTauri ? invoke<LlmStatus>('llm_configure', { config }) : Promise.resolve(null)),
  /** Rejects with the reason (empty or invalid key, Credential Manager error). */
  setKey: (key: string): Promise<LlmStatus | null> => (isTauri ? invoke<LlmStatus>('llm_set_key', { key }) : Promise.resolve(null)),
  clearKey: (): Promise<LlmStatus | null> => (isTauri ? invoke<LlmStatus>('llm_clear_key') : Promise.resolve(null)),
  /** Translates a sample sentence with the current settings, even while the switch is off. */
  test: (): Promise<LlmTest | null> => (isTauri ? invoke<LlmTest>('llm_test') : Promise.resolve(null)),
  ollamaLocal: (): Promise<OllamaLocal | null> => (isTauri ? invoke<OllamaLocal>('llm_ollama_local') : Promise.resolve(null)),
  /** Opens the service's key page (or Ollama's download page) in the browser. */
  openKeyPage: (service: LlmService | 'ollama-download'): Promise<void> => (isTauri ? invoke<void>('llm_open_key_page', { service }) : Promise.resolve()),
};
