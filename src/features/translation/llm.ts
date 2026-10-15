// The translation engine (Audio & IA › Traduction IA): a cloud provider with the
// user's key, or Ollama / LM Studio on this PC, mirrored from the Rust core. Not
// connected until the user chooses; nothing else translates.

import { create } from 'zustand';
import { isTauri, llm, llmService, type LlmConfig, type LlmProvider, type LlmService, type LlmStatus, type LlmModels, type LlmTest, type LocalServer, type Translation } from '../../shared/ipc';
import type { AppState } from '../../app/state/store';

export interface LlmState {
  native: boolean;
  status: LlmStatus | null;
  /** Last connection or translation test, and the settings it was run with. */
  test: LlmTest | null;
  testedWith: string | null;
  testing: boolean;
  /** Ollama and LM Studio on this PC, null until detected. */
  local: LocalServer[] | null;
  detecting: boolean;
  /** Models of the configured server, and the settings they were listed with. */
  models: LlmModels | null;
  modelsFor: string | null;
  loadingModels: boolean;
}

export const useLlm = create<LlmState>()(() => ({ native: isTauri, status: null, test: null, testedWith: null, testing: false, local: null, detecting: false, models: null, modelsFor: null, loadingModels: false }));

/** Rust LLM settings from the UI (the key is never part of them). */
export function llmConfigFrom(s: Pick<AppState, 'llm'>): LlmConfig {
  return { enabled: s.llm.enabled, provider: s.llm.provider, baseUrl: s.llm.baseUrl.trim(), model: s.llm.model.trim() };
}

/** Settings a test result belongs to (connecting or disconnecting keeps it shown). */
export const testKey = (c: LlmConfig): string => JSON.stringify({ provider: c.provider, baseUrl: c.baseUrl, model: c.model });

/** Where the settings send texts: a cloud provider, or a server on this PC / the local network. */
export function isLocalChoice(opts: Pick<AppState['llm'], 'service' | 'baseUrl'>): boolean {
  const info = llmService(opts.service);
  if (info) return info.local;
  try {
    const host = new URL(opts.baseUrl.trim()).hostname.replace(/^\[|\]$/g, '');
    return host === 'localhost' || host === '::1' || /^127\./.test(host) || /^10\./.test(host) || /^192\.168\./.test(host) || /^172\.(1[6-9]|2\d|3[01])\./.test(host) || host.endsWith('.local');
  } catch {
    return false;
  }
}

/**
 * Choosing a service fills its address and suggested model. For a server on this PC the
 * model comes from the installed ones; "Autre" keeps the current address and model.
 */
export function serviceChange(next: LlmService, current: AppState['llm'], localModels: string[] = []): Partial<AppState['llm']> {
  if (next === 'custom') return { service: 'custom' };
  const info = llmService(next);
  if (!info) return {};
  let model = info.model;
  if (info.local) model = localModels.includes(current.model) ? current.model : (localModels[0] ?? '');
  return { service: next, provider: info.provider, baseUrl: info.baseUrl, model };
}

/** The detected server for a local service, if any. */
export function localServer(local: LocalServer[] | null, service: LlmService): LocalServer | null {
  if (service !== 'ollama' && service !== 'lmstudio') return null;
  return local?.find((l) => l.kind === service) ?? null;
}

/** What to do after a failed connection test, in plain words. */
export function testAdvice(error: string, service: LlmService): string {
  const e = error.toLowerCase();
  const local = llmService(service)?.local ?? false;
  if (e.includes('clé api non enregistrée')) return 'Collez votre clé, puis cliquez sur « Enregistrer ».';
  if (e.includes('modèle non renseigné')) return local ? 'Choisissez un modèle installé (ou installez-en un), puis testez à nouveau.' : 'Choisissez un modèle dans la liste, ou saisissez son nom.';
  if (e.includes('clé refusée')) return 'La clé est incorrecte, expirée ou appartient à un autre service. Créez-en une nouvelle sur la page du service et collez-la à nouveau.';
  if (e.includes('402')) return 'Le compte n’a plus de crédit : ajoutez un moyen de paiement ou du crédit sur le site du service.';
  if (e.includes('429')) return 'Trop de demandes ou quota gratuit épuisé : attendez quelques minutes ou vérifiez votre forfait.';
  if (e.includes('404')) return 'Ce modèle n’existe pas chez ce service. Choisissez-en un dans la liste du service.';
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
  useLlm.setState({ testing: true, test: null, testedWith: testKey(config) });
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

/**
 * Translates the user's own sentence with the settings on screen (Audio & IA › Tester une
 * traduction). Not counted in the statistics.
 */
export async function runTranslateTest(config: LlmConfig, text: string, source: string, target: string): Promise<void> {
  useLlm.setState({ testing: true, test: null, testedWith: testKey(config) });
  try {
    const status = await llm.configure(config);
    if (status) useLlm.setState({ status });
    const test = await llm.translateTest(text, source, target);
    useLlm.setState({ test });
  } catch (e) {
    useLlm.setState({ test: { ok: false, ms: 0, original: text, translated: null, error: String(e) } });
  } finally {
    useLlm.setState({ testing: false });
    refreshLlm();
  }
}

/** Looks for Ollama (localhost:11434) and LM Studio (localhost:1234). Never installs anything. */
export async function detectLocal(): Promise<LocalServer[]> {
  useLlm.setState({ detecting: true });
  try {
    const local = await llm.localServers();
    useLlm.setState({ local });
    return local;
  } catch {
    const local: LocalServer[] = [];
    useLlm.setState({ local });
    return local;
  } finally {
    useLlm.setState({ detecting: false });
  }
}

/** Lists the models of the configured server (the settings on screen are sent first). */
export async function loadModels(config: LlmConfig): Promise<LlmModels | null> {
  const key = `${config.provider}|${config.baseUrl}`;
  useLlm.setState({ loadingModels: true, modelsFor: key });
  try {
    const status = await llm.configure(config);
    if (status) useLlm.setState({ status });
    const models = await llm.models();
    useLlm.setState({ models });
    return models;
  } catch (e) {
    const models = { models: [], error: String(e) };
    useLlm.setState({ models });
    return models;
  } finally {
    useLlm.setState({ loadingModels: false });
  }
}

export const MODEL_EXAMPLES: Record<LlmProvider, string> = {
  openai: 'gpt-4o-mini, mistral-small-latest, qwen2.5:7b…',
  anthropic: 'claude-haiku-5-5, claude-haiku-4-5…',
};

export interface LlmSummary {
  value: string;
  color: string;
}

/** "Connecté", "Problème : …" or "Non connecté", for the card. */
export function llmSummary(st: LlmStatus | null, enabled: boolean): LlmSummary {
  if (!enabled) return { value: 'Non connecté · traduction indisponible', color: '#8f8a83' };
  if (!st) return { value: '…', color: '#8f8a83' };
  if (st.active) return { value: 'Connecté', color: '#7fe0a8' };
  if (st.pausedS) return { value: `En pause ${st.pausedS} s (quota ou limite du service)`, color: '#ffa24a' };
  return { value: `Problème : ${st.problem ?? 'inconnu'}`, color: '#ffa24a' };
}

/** "8 traductions · 12 appels · 1 échec · dernière latence 640 ms". */
export function llmStatsLine(st: LlmStatus): string {
  const n = st.translations ?? 0;
  const parts = [`${n} traduction${n > 1 ? 's' : ''}`, `${st.calls} appel${st.calls > 1 ? 's' : ''}`, `${st.failures} échec${st.failures > 1 ? 's' : ''}`];
  if (st.lastLatencyMs != null) parts.push(`dernière latence ${st.lastLatencyMs} ms`);
  return parts.join(' · ');
}

/** Engine of a translated line, as shown after the manual test. */
export function engineLabel(engine: Translation['engine']): string | null {
  switch (engine) {
    case 'llm':
      return 'LLM';
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
