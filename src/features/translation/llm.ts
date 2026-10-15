// Online translation by an LLM (Audio & IA › Traduction en ligne), mirrored from
// the Rust core. Off by default; Opus-MT takes over on any failure.

import { create } from 'zustand';
import { isTauri, llm, LLM_DEFAULT_URL, llmService, type LlmConfig, type LlmProvider, type LlmService, type LlmStatus, type LlmTest, type OllamaLocal, type Translation } from '../../shared/ipc';
import type { AppState } from '../../app/state/store';

export interface LlmState {
  native: boolean;
  status: LlmStatus | null;
  /** Last "Tester la connexion" result, and the settings it was run with. */
  test: LlmTest | null;
  testedWith: string | null;
  testing: boolean;
  /** Ollama on this PC, null until checked. */
  ollama: OllamaLocal | null;
  checkingOllama: boolean;
}

export const useLlm = create<LlmState>()(() => ({ native: isTauri, status: null, test: null, testedWith: null, testing: false, ollama: null, checkingOllama: false }));

/** Rust LLM settings from the UI (the key is never part of them). */
export function llmConfigFrom(s: Pick<AppState, 'llm'>): LlmConfig {
  return { enabled: s.llm.enabled, provider: s.llm.provider, baseUrl: s.llm.baseUrl.trim(), model: s.llm.model.trim() };
}

/**
 * Choosing a service fills its address and suggested model. For Ollama on this PC the
 * model comes from the installed ones; "Autre" keeps the current address and model.
 */
export function serviceChange(next: LlmService, current: AppState['llm'], ollamaModels: string[] = []): Partial<AppState['llm']> {
  if (next === 'custom') return { service: 'custom' };
  const info = llmService(next);
  if (!info) return {};
  let model = info.model;
  if (next === 'ollama') model = ollamaModels.includes(current.model) ? current.model : (ollamaModels[0] ?? '');
  return { service: next, provider: info.provider, baseUrl: info.baseUrl, model };
}

/** What to do after a failed connection test, in plain words. */
export function testAdvice(error: string, service: LlmService): string {
  const e = error.toLowerCase();
  const local = llmService(service)?.local ?? false;
  if (e.includes('clé api non enregistrée')) return 'Collez votre clé à l’étape 3, puis cliquez sur « Enregistrer ».';
  if (e.includes('modèle non renseigné')) return local ? 'Choisissez un modèle installé (ou installez-en un), puis testez à nouveau.' : 'Indiquez un modèle dans « Réglages avancés ».';
  if (e.includes('clé refusée')) return 'La clé est incorrecte, expirée ou appartient à un autre service. Créez-en une nouvelle sur la page du service et collez-la à nouveau.';
  if (e.includes('402')) return 'Le compte n’a plus de crédit : ajoutez un moyen de paiement ou du crédit sur le site du service.';
  if (e.includes('429')) return 'Trop de demandes ou quota gratuit épuisé : attendez quelques minutes ou vérifiez votre forfait.';
  if (e.includes('404')) return 'Ce modèle n’existe pas chez ce service. Vérifiez son nom dans « Réglages avancés ».';
  if (e.includes('réseau')) return local ? 'Le serveur local ne répond pas : lancez-le (Ollama ou LM Studio), puis testez à nouveau.' : 'Vérifiez votre connexion Internet, votre pare-feu ou votre VPN.';
  if (e.includes('délai')) return local ? 'Le modèle local met du temps à démarrer : réessayez dans quelques secondes, ou choisissez un modèle plus léger.' : 'Le service est lent en ce moment : réessayez, ou choisissez un modèle plus rapide.';
  if (e.includes('adresse')) return 'L’adresse doit commencer par https:// (http:// seulement pour un serveur sur ce PC ou le réseau local).';
  if (e.includes('incohérente')) return 'Le modèle a répondu autre chose qu’une traduction : essayez un modèle plus récent ou plus grand.';
  return 'Vérifiez le service, le modèle et la clé, puis testez à nouveau.';
}

/**
 * Runs "Tester la connexion": the settings on screen are sent first (the bridge waits
 * 400 ms after typing), then a sample sentence goes to the service.
 */
export async function runLlmTest(config: LlmConfig): Promise<void> {
  useLlm.setState({ testing: true, test: null, testedWith: JSON.stringify(config) });
  try {
    const status = await llm.configure(config);
    if (status) useLlm.setState({ status });
    const test = await llm.test();
    useLlm.setState({ test });
  } catch (e) {
    useLlm.setState({ test: { ok: false, ms: 0, original: '', translated: null, error: String(e) } });
  } finally {
    useLlm.setState({ testing: false });
    refreshLlm();
  }
}

/** Looks for Ollama on this PC (http://localhost:11434). */
export async function checkOllama(): Promise<OllamaLocal | null> {
  useLlm.setState({ checkingOllama: true });
  try {
    const ollama = await llm.ollamaLocal();
    useLlm.setState({ ollama });
    return ollama;
  } catch {
    const ollama = { running: false, models: [] };
    useLlm.setState({ ollama });
    return ollama;
  } finally {
    useLlm.setState({ checkingOllama: false });
  }
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
