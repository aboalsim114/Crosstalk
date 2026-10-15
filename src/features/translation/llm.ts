// Online translation by an LLM (Audio & IA › Traduction en ligne), mirrored from
// the Rust core. Off by default; Opus-MT takes over on any failure.

import { create } from 'zustand';
import { isTauri, llm, LLM_DEFAULT_URL, type LlmConfig, type LlmProvider, type LlmStatus, type Translation } from '../../shared/ipc';
import type { AppState } from '../../app/state/store';

export interface LlmState {
  native: boolean;
  status: LlmStatus | null;
}

export const useLlm = create<LlmState>()(() => ({ native: isTauri, status: null }));

/** Rust LLM settings from the UI (the key is never part of them). */
export function llmConfigFrom(s: Pick<AppState, 'llm'>): LlmConfig {
  return { enabled: s.llm.enabled, provider: s.llm.provider, baseUrl: s.llm.baseUrl.trim(), model: s.llm.model.trim() };
}

/** Changing provider pre-fills its address, unless the user typed their own. */
export function providerChange(next: LlmProvider, current: Pick<LlmConfig, 'provider' | 'baseUrl'>): Pick<LlmConfig, 'provider' | 'baseUrl'> {
  const typed = current.baseUrl.trim();
  const isDefault = typed === '' || typed === LLM_DEFAULT_URL[current.provider];
  return { provider: next, baseUrl: isDefault ? LLM_DEFAULT_URL[next] : current.baseUrl };
}

export const MODEL_EXAMPLES: Record<LlmProvider, string> = {
  openai: 'gpt-4o-mini, mistral-small-latest, qwen2.5:7b…',
  anthropic: 'claude-haiku-5-5, claude-haiku-4-5…',
};

export interface LlmSummary {
  value: string;
  color: string;
}

/** "Actif", "Problème : …" or "Désactivé", for the card. */
export function llmSummary(st: LlmStatus | null, enabled: boolean): LlmSummary {
  if (!enabled) return { value: 'Désactivé · Opus-MT local seulement', color: '#8f8a83' };
  if (!st) return { value: '…', color: '#8f8a83' };
  if (st.active) return { value: 'Actif · Opus-MT en secours', color: '#7fe0a8' };
  return { value: `Problème : ${st.problem ?? 'inconnu'} · Opus-MT utilisé`, color: '#ffa24a' };
}

/** "12 appels · 1 échec · dernière latence 640 ms". */
export function llmStatsLine(st: LlmStatus): string {
  const parts = [`${st.calls} appel${st.calls > 1 ? 's' : ''}`, `${st.failures} échec${st.failures > 1 ? 's' : ''}`];
  if (st.lastLatencyMs != null) parts.push(`dernière latence ${st.lastLatencyMs} ms`);
  return parts.join(' · ');
}

/** Engine of a translated line, as shown after the manual test. */
export function engineLabel(engine: Translation['engine']): string | null {
  switch (engine) {
    case 'llm':
      return 'LLM en ligne';
    case 'opus-mt':
      return 'Opus-MT local';
    case 'lexicon':
      return 'lexique Counter-Strike';
    default:
      return null;
  }
}

export async function refreshLlm(): Promise<void> {
  const status = await llm.status();
  if (status) useLlm.setState({ status });
}

/** Sends the key to the Rust core (Credential Manager). Returns an error message, or null. */
export async function saveLlmKey(key: string): Promise<string | null> {
  try {
    const status = await llm.setKey(key);
    if (status) useLlm.setState({ status });
    return null;
  } catch (e) {
    return String(e);
  }
}

export async function clearLlmKey(): Promise<string | null> {
  try {
    const status = await llm.clearKey();
    if (status) useLlm.setState({ status });
    return null;
  } catch (e) {
    return String(e);
  }
}
