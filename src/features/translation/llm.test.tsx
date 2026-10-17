import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { llm, LLM_DEFAULT_URL, LLM_SERVICES, serviceOf, type LlmStatus, type LocalServer } from '../../shared/ipc';
import { LLM_DEFAULTS, useApp } from '../../app/state/store';
import { engineLabel, isLocalChoice, llmConfigFrom, llmStatsLine, llmSummary, localServer, serviceChange, testAdvice, testKey, useLlm } from './llm';
import { LlmCard, localState, TRY_SENTENCE } from './LlmCard';

// Simulated statuses and servers: no real provider or local server is called in these tests.
const status = (p: Partial<LlmStatus> = {}): LlmStatus => ({
  config: { enabled: true, provider: 'openai', baseUrl: LLM_DEFAULT_URL.openai, model: 'gpt-4o-mini' },
  kind: 'cloud',
  backend: null,
  hasKey: true,
  active: true,
  problem: null,
  pausedS: null,
  keyStorage: 'Gestionnaire d’identifiants Windows',
  calls: 0,
  failures: 0,
  translations: 0,
  lastLatencyMs: null,
  lastError: null,
  ...p,
});
const ollamaUp: LocalServer = { kind: 'ollama', baseUrl: 'http://localhost:11434/v1', running: true, models: ['llama3.2:3b', 'translategemma:4b'] };
const lmDown: LocalServer = { kind: 'lmstudio', baseUrl: 'http://localhost:1234/v1', running: false, models: [] };

const app0 = useApp.getState();
const llm0 = useLlm.getState();
beforeEach(() => {
  useApp.setState(app0, true);
  useLlm.setState(llm0, true);
});
afterEach(() => vi.restoreAllMocks());

describe('translation engine settings', () => {
  it('is not connected by default and sends trimmed settings, never a key', () => {
    expect(LLM_DEFAULTS.enabled).toBe(false);
    expect(LLM_DEFAULTS.cloudConsent).toBe(false);
    const c = llmConfigFrom({ llm: { enabled: true, service: 'anthropic', provider: 'anthropic', baseUrl: ' https://api.anthropic.com/v1 ', model: ' claude-haiku-4-5 ', cloudConsent: true } });
    expect(c).toEqual({ enabled: true, provider: 'anthropic', baseUrl: 'https://api.anthropic.com/v1', model: 'claude-haiku-4-5' });
    expect(Object.keys(c)).not.toContain('key');
    expect(Object.keys(c)).not.toContain('cloudConsent');
  });

  it('tells a server on this PC or the local network from a cloud provider', () => {
    expect(isLocalChoice({ service: 'ollama', baseUrl: '' })).toBe(true);
    expect(isLocalChoice({ service: 'lmstudio', baseUrl: '' })).toBe(true);
    expect(isLocalChoice({ service: 'groq', baseUrl: '' })).toBe(false);
    expect(isLocalChoice({ service: 'custom', baseUrl: 'http://192.168.1.20:8080/v1' })).toBe(true);
    expect(isLocalChoice({ service: 'custom', baseUrl: 'http://[::1]:8080/v1' })).toBe(true);
    expect(isLocalChoice({ service: 'custom', baseUrl: 'https://api.together.xyz/v1' })).toBe(false);
    expect(isLocalChoice({ service: 'custom', baseUrl: 'pas une adresse' })).toBe(false);
  });

  it('summarises the state and the counters, with no other engine behind', () => {
    expect(llmSummary(null, false).value).toBe('Non connecté · traduction indisponible');
    expect(llmSummary(status(), true)).toEqual({ value: 'Connecté', color: '#7fe0a8' });
    expect(llmSummary(status({ active: false, problem: 'clé API non enregistrée' }), true).value).toBe('Problème : clé API non enregistrée');
    expect(llmSummary(status({ active: false, pausedS: 18, problem: 'quota' }), true).value).toMatch(/^En pause 18 s/);
    expect(llmStatsLine(status({ calls: 12, failures: 1, translations: 11, lastLatencyMs: 640 }))).toBe('11 traductions · 12 appels · 1 échec · dernière latence 640 ms');
    expect(llmStatsLine(status({ calls: 1, failures: 0, translations: 1 }))).toBe('1 traduction · 1 appel · 0 échec');
    expect(engineLabel('llm')).toBe('LLM');
    expect(engineLabel('lexicon')).toBe('lexique Counter-Strike');
    expect(engineLabel(null)).toBeNull();
    for (const s of [llmSummary(null, false), llmSummary(status(), true), llmSummary(status({ active: false }), true)]) expect(s.value).not.toMatch(/opus/i);
  });

  it('keeps a test result across connect and disconnect, not across other settings', () => {
    const c = llmConfigFrom({ llm: LLM_DEFAULTS });
    expect(testKey({ ...c, enabled: true })).toBe(testKey(c));
    expect(testKey({ ...c, model: 'gpt-4.1-mini' })).not.toBe(testKey(c));
  });
});

describe('ready-made services', () => {
  it('fills address and model, Ollama Cloud with a key, local servers with an installed model', () => {
    const cloud = serviceChange('ollama-cloud', LLM_DEFAULTS);
    expect(cloud).toEqual({ service: 'ollama-cloud', provider: 'openai', baseUrl: 'https://ollama.com/v1', model: 'gemma4:31b' });
    expect(LLM_SERVICES.find((s) => s.id === 'ollama-cloud')?.local).toBe(false);
    expect(serviceChange('anthropic', LLM_DEFAULTS)).toMatchObject({ provider: 'anthropic', baseUrl: 'https://api.anthropic.com/v1' });
    expect(serviceChange('ollama', LLM_DEFAULTS, ['llama3.2:3b', 'qwen2.5:7b'])).toEqual({ service: 'ollama', provider: 'openai', baseUrl: 'http://localhost:11434/v1', model: 'llama3.2:3b' });
    expect(serviceChange('ollama', { ...LLM_DEFAULTS, model: 'qwen2.5:7b' }, ['llama3.2:3b', 'qwen2.5:7b']).model).toBe('qwen2.5:7b');
    expect(serviceChange('lmstudio', LLM_DEFAULTS, ['qwen2.5-7b-instruct'])).toMatchObject({ baseUrl: 'http://localhost:1234/v1', model: 'qwen2.5-7b-instruct' });
    // Nothing installed: no model is invented.
    expect(serviceChange('ollama', LLM_DEFAULTS).model).toBe('');
    expect(serviceChange('custom', LLM_DEFAULTS)).toEqual({ service: 'custom' });
    for (const s of LLM_SERVICES) {
      expect(s.baseUrl.startsWith('https://') || s.local).toBe(true);
      expect(serviceOf(s)).toBe(s.id);
    }
    // Every cloud provider of the request is offered.
    expect(LLM_SERVICES.filter((s) => !s.local).map((s) => s.id)).toEqual(['openai', 'openrouter', 'mistral', 'groq', 'anthropic', 'ollama-cloud']);
  });

  it('describes a local server: running with models, running empty, absent', () => {
    expect(localServer([ollamaUp, lmDown], 'ollama')).toBe(ollamaUp);
    expect(localServer([ollamaUp, lmDown], 'groq')).toBeNull();
    expect(localState(ollamaUp, true)).toEqual({ text: 'Détecté · 2 modèles', color: '#7fe0a8' });
    expect(localState({ ...ollamaUp, models: [] }, true).text).toBe('Démarré, aucun modèle disponible');
    expect(localState(lmDown, true).text).toBe('Non détecté sur ce PC');
    expect(localState(null, false).text).toBe('Recherche…');
  });

  it('turns test errors into plain advice', () => {
    expect(testAdvice('clé refusée par le fournisseur', 'openai')).toMatch(/Créez-en une nouvelle/);
    expect(testAdvice('clé API non enregistrée', 'groq')).toMatch(/Collez votre clé/);
    expect(testAdvice('crédit épuisé chez le fournisseur (402)', 'openai')).toMatch(/crédit/);
    expect(testAdvice('quota ou limite de débit atteint (429)', 'openrouter')).toMatch(/quota/);
    expect(testAdvice('modèle ou adresse introuvable (404)', 'mistral')).toMatch(/liste du service/);
    expect(testAdvice('réseau indisponible (connexion refusée)', 'ollama')).toMatch(/serveur local ne répond pas/);
    expect(testAdvice('réseau indisponible (dns)', 'openai')).toMatch(/connexion Internet/);
    expect(testAdvice('modèle non renseigné', 'ollama')).toMatch(/modèle installé/);
    expect(testAdvice('délai dépassé (6.0 s)', 'anthropic')).toMatch(/lent/);
  });
});

describe('TRADUCTION IA card', () => {
  it('shows Cloud and Local, the providers, the key, the consent and the state', () => {
    render(<LlmCard />);
    expect(screen.getByText('TRADUCTION IA')).toBeInTheDocument();
    expect(screen.getByTestId('llm-badge')).toHaveTextContent('NON CONNECTÉ');
    expect(screen.getByRole('radio', { name: 'Cloud — Fournisseur API' })).toHaveAttribute('aria-checked', 'true');
    expect(screen.getByRole('radio', { name: 'Local — LLM installé sur ce PC' })).toBeInTheDocument();
    for (const name of ['OpenAI', 'OpenRouter', 'Mistral AI', 'Groq', 'Anthropic', 'Ollama Cloud', 'Autre compatible OpenAI']) expect(screen.getByRole('radio', { name: new RegExp(`^${name}`) })).toBeInTheDocument();
    expect(screen.getByText('Obtenir ma clé OpenAI ↗')).toBeInTheDocument();
    expect(screen.getByRole('textbox', { name: 'Modèle' })).toHaveValue('gpt-4o-mini');
    expect(screen.getByTestId('llm-consent')).toHaveTextContent('Jamais l’audio, jamais les pseudos des joueurs');
    expect(screen.getByTestId('llm-state')).toHaveTextContent('Non connecté · traduction indisponible');
    expect(screen.getByRole('textbox', { name: 'Phrase à essayer' })).toHaveValue(TRY_SENTENCE);
    expect(document.body.textContent).not.toMatch(/opus/i);
    // Outside the desktop app the key cannot be stored.
    expect(screen.getByLabelText('Clé API')).toBeDisabled();
  });

  it('refuses to connect a cloud provider before the consent, then connects and disconnects', () => {
    useLlm.setState({ native: true, status: status({ active: false }) });
    render(<LlmCard />);
    fireEvent.click(screen.getByText('Connecter'));
    expect(screen.getByTestId('llm-connect-error')).toHaveTextContent('Acceptez d’abord l’envoi du texte des phrases à OpenAI');
    expect(useApp.getState().llm.enabled).toBe(false);
    fireEvent.click(screen.getByTestId('llm-consent'));
    expect(useApp.getState().llm.cloudConsent).toBe(true);
    fireEvent.click(screen.getByText('Connecter'));
    expect(useApp.getState().llm.enabled).toBe(true);
    fireEvent.click(screen.getByText('Déconnecter'));
    expect(useApp.getState().llm.enabled).toBe(false);
  });

  it('asks for a key before connecting a cloud provider', () => {
    useLlm.setState({ native: true, status: status({ hasKey: false, active: false }) });
    useApp.setState({ llm: { ...LLM_DEFAULTS, cloudConsent: true } });
    render(<LlmCard />);
    fireEvent.click(screen.getByText('Connecter'));
    expect(screen.getByTestId('llm-connect-error')).toHaveTextContent('Enregistrez d’abord votre clé API');
    expect(useApp.getState().llm.enabled).toBe(false);
  });

  it('withdrawing the consent disconnects', () => {
    useApp.setState({ llm: { ...LLM_DEFAULTS, enabled: true, cloudConsent: true } });
    render(<LlmCard />);
    fireEvent.click(screen.getByTestId('llm-consent'));
    expect(useApp.getState().llm).toMatchObject({ enabled: false, cloudConsent: false });
  });

  it('a provider change disconnects and fills its settings', () => {
    useApp.setState({ llm: { ...LLM_DEFAULTS, enabled: true, cloudConsent: true } });
    render(<LlmCard />);
    fireEvent.click(screen.getByRole('radio', { name: /Anthropic/ }));
    expect(useApp.getState().llm).toMatchObject({ service: 'anthropic', provider: 'anthropic', model: 'claude-haiku-5-5', enabled: false });
    fireEvent.change(screen.getByRole('textbox', { name: 'Modèle' }), { target: { value: 'claude-haiku-4-5' } });
    expect(useApp.getState().llm.model).toBe('claude-haiku-4-5');
  });

  it('sends the key once, empties the field and never shows it again', async () => {
    useLlm.setState({ native: true, status: status({ hasKey: false, active: false, problem: 'clé API non enregistrée' }) });
    const setKey = vi.spyOn(llm, 'setKey').mockResolvedValue(status());
    render(<LlmCard />);
    const field = screen.getByLabelText('Clé API');
    expect(field).toHaveAttribute('type', 'password');
    fireEvent.change(field, { target: { value: 'sk-test-placeholder' } });
    await act(async () => fireEvent.click(screen.getByText('Enregistrer')));
    expect(setKey).toHaveBeenCalledWith('sk-test-placeholder');
    await waitFor(() => expect(field).toHaveValue(''));
    expect(screen.getByTestId('llm-key-state')).toHaveTextContent('Clé enregistrée · Gestionnaire d’identifiants Windows');
    expect(document.body.textContent).not.toContain('sk-test-placeholder');
    expect(JSON.stringify(useApp.getState())).not.toContain('sk-test-placeholder');
  });

  it('shows why a key was refused and lets the user erase a stored one', async () => {
    useLlm.setState({ native: true, status: status() });
    useApp.setState({ llm: { ...LLM_DEFAULTS, enabled: true, cloudConsent: true } });
    vi.spyOn(llm, 'setKey').mockRejectedValue('clé invalide (un seul mot, sans espace)');
    const clearKey = vi.spyOn(llm, 'clearKey').mockResolvedValue(status({ hasKey: false, active: false, problem: 'clé API non enregistrée' }));
    render(<LlmCard />);
    fireEvent.change(screen.getByLabelText('Clé API'), { target: { value: 'two words' } });
    await act(async () => fireEvent.click(screen.getByText('Enregistrer')));
    expect(await screen.findByRole('alert')).toHaveTextContent('clé invalide (un seul mot, sans espace)');
    await act(async () => fireEvent.click(screen.getByText('Effacer')));
    expect(clearKey).toHaveBeenCalled();
    expect(screen.getByTestId('llm-key-state')).toHaveTextContent('Aucune clé enregistrée');
    expect(screen.getByTestId('llm-state')).toHaveTextContent('Problème : clé API non enregistrée');
  });

  it('shows provider, model, type, languages, activity and last error', () => {
    useLlm.setState({ native: true, status: status({ calls: 7, failures: 2, translations: 5, lastLatencyMs: 812, lastError: 'quota ou limite de débit atteint (429)' }) });
    useApp.setState({ llm: { ...LLM_DEFAULTS, enabled: true, cloudConsent: true } });
    render(<LlmCard />);
    const state = screen.getByTestId('llm-state');
    expect(state).toHaveTextContent('Connecté');
    expect(state).toHaveTextContent('OpenAI');
    expect(state).toHaveTextContent('gpt-4o-mini');
    expect(state).toHaveTextContent('Cloud (texte envoyé au fournisseur)');
    expect(state).toHaveTextContent('détectées automatiquement → FR');
    expect(state).toHaveTextContent('5 traductions · 7 appels · 2 échecs · dernière latence 812 ms');
    expect(state).toHaveTextContent('quota ou limite de débit atteint (429)');
    expect(screen.getByTestId('llm-badge')).toHaveTextContent('CLOUD · CONNECTÉ');
  });

  it('lists the provider’s models and lets the user pick one or type another', async () => {
    useLlm.setState({ native: true, status: status() });
    vi.spyOn(llm, 'configure').mockResolvedValue(status());
    const models = vi.spyOn(llm, 'models').mockResolvedValue({ models: ['gpt-4.1-mini', 'gpt-4o-mini'], error: null });
    render(<LlmCard />);
    await act(async () => fireEvent.click(screen.getByText('Lister les modèles')));
    expect(models).toHaveBeenCalled();
    fireEvent.change(await screen.findByLabelText('Modèle disponible'), { target: { value: 'gpt-4.1-mini' } });
    expect(useApp.getState().llm.model).toBe('gpt-4.1-mini');
  });

  it('says why the model list is unavailable', async () => {
    useLlm.setState({ native: true, status: status({ hasKey: false }) });
    vi.spyOn(llm, 'configure').mockResolvedValue(status());
    vi.spyOn(llm, 'models').mockResolvedValue({ models: [], error: 'clé API non enregistrée' });
    render(<LlmCard />);
    await act(async () => fireEvent.click(screen.getByText('Lister les modèles')));
    expect(await screen.findByText('Liste indisponible : clé API non enregistrée')).toBeInTheDocument();
  });

  it('opens the key page of the chosen service, Ollama Cloud included', () => {
    useLlm.setState({ native: true, status: status({ hasKey: false }) });
    const open = vi.spyOn(llm, 'openKeyPage').mockResolvedValue();
    render(<LlmCard />);
    fireEvent.click(screen.getByRole('radio', { name: /Ollama Cloud/ }));
    expect(useApp.getState().llm.baseUrl).toBe('https://ollama.com/v1');
    fireEvent.click(screen.getByText('Obtenir ma clé Ollama Cloud ↗'));
    expect(open).toHaveBeenCalledWith('ollama-cloud');
    expect(screen.getByLabelText('Clé API')).toBeEnabled();
  });

  it('Local: detects Ollama and LM Studio, picks an installed model and never asks for a key', async () => {
    useLlm.setState({ native: true, status: status({ hasKey: false, kind: 'local', backend: 'Ollama' }) });
    const detect = vi.spyOn(llm, 'localServers').mockResolvedValue([ollamaUp, lmDown]);
    render(<LlmCard />);
    await act(async () => fireEvent.click(screen.getByRole('radio', { name: 'Local — LLM installé sur ce PC' })));
    expect(detect).toHaveBeenCalled();
    expect(await screen.findByTestId('llm-local-ollama')).toHaveTextContent('Détecté · 2 modèles');
    expect(screen.getByTestId('llm-local-lmstudio')).toHaveTextContent('Non détecté sur ce PC');
    expect(useApp.getState().llm).toMatchObject({ service: 'ollama', baseUrl: 'http://localhost:11434/v1', enabled: false });
    fireEvent.change(screen.getByLabelText('Modèle disponible'), { target: { value: 'translategemma:4b' } });
    expect(useApp.getState().llm.model).toBe('translategemma:4b');
    expect(screen.queryByLabelText('Clé API')).toBeNull();
    expect(screen.queryByTestId('llm-consent')).toBeNull();
    // No key and no consent needed to connect a local server.
    fireEvent.click(screen.getByText('Connecter'));
    expect(useApp.getState().llm.enabled).toBe(true);
    expect(screen.getByTestId('llm-state')).toHaveTextContent('Local (rien ne quitte ce PC ou le réseau local)');
  });

  it('Local: explains a missing server without downloading anything, then detects again', async () => {
    useLlm.setState({ native: true, status: status({ hasKey: false }) });
    const detect = vi.spyOn(llm, 'localServers').mockResolvedValue([{ ...ollamaUp, running: false, models: [] }, lmDown]);
    const open = vi.spyOn(llm, 'openKeyPage').mockResolvedValue();
    render(<LlmCard />);
    await act(async () => fireEvent.click(screen.getByRole('radio', { name: 'Local — LLM installé sur ce PC' })));
    expect(await screen.findByTestId('llm-local-help')).toHaveTextContent('Crosstalk ne télécharge aucun modèle');
    fireEvent.click(screen.getByText('Télécharger Ollama ↗'));
    expect(open).toHaveBeenCalledWith('ollama-download');
    fireEvent.click(screen.getByTestId('llm-local-lmstudio'));
    fireEvent.click(screen.getByText('Télécharger LM Studio ↗'));
    expect(open).toHaveBeenCalledWith('lmstudio-download');
    // Started without a model.
    detect.mockResolvedValue([{ ...ollamaUp, models: [] }, { ...lmDown, running: true }]);
    await act(async () => fireEvent.click(screen.getByText('Actualiser les modèles locaux')));
    expect(await screen.findByTestId('llm-local-empty')).toHaveTextContent('Aucun modèle chargé dans LM Studio');
  });

  it('Local: refuses to connect without a model', async () => {
    useLlm.setState({ native: true, status: status({ hasKey: false }) });
    vi.spyOn(llm, 'localServers').mockResolvedValue([{ ...ollamaUp, models: [] }, lmDown]);
    render(<LlmCard />);
    await act(async () => fireEvent.click(screen.getByRole('radio', { name: 'Local — LLM installé sur ce PC' })));
    fireEvent.click(screen.getByText('Connecter'));
    expect(screen.getByTestId('llm-connect-error')).toHaveTextContent('Choisissez un modèle');
    expect(useApp.getState().llm.enabled).toBe(false);
  });

  it('Local: a manual address on the local network', async () => {
    useLlm.setState({ native: true, status: status({ hasKey: false }) });
    vi.spyOn(llm, 'localServers').mockResolvedValue([]);
    render(<LlmCard />);
    await act(async () => fireEvent.click(screen.getByRole('radio', { name: 'Local — LLM installé sur ce PC' })));
    fireEvent.click(screen.getByRole('radio', { name: /Adresse manuelle/ }));
    fireEvent.change(screen.getByRole('textbox', { name: 'Adresse de base' }), { target: { value: 'http://192.168.1.20:8080/v1' } });
    expect(useApp.getState().llm).toMatchObject({ service: 'custom', baseUrl: 'http://192.168.1.20:8080/v1' });
    expect(isLocalChoice(useApp.getState().llm)).toBe(true);
    expect(screen.queryByLabelText('Clé API')).toBeNull();
  });

  it('tests the connection with the settings on screen and shows the sample translation', async () => {
    useLlm.setState({ native: true, status: status() });
    const configure = vi.spyOn(llm, 'configure').mockResolvedValue(status());
    vi.spyOn(llm, 'status').mockResolvedValue(status());
    vi.spyOn(llm, 'test').mockResolvedValue({ ok: true, ms: 640, original: 'Двое идут на B', translated: 'Deux vont sur B', error: null });
    render(<LlmCard />);
    await act(async () => fireEvent.click(screen.getByText('Tester la connexion')));
    expect(configure).toHaveBeenCalledWith(llmConfigFrom(useApp.getState()));
    expect(await screen.findByTestId('llm-test')).toHaveTextContent('✓ Réponse en 640 ms');
    expect(screen.getByTestId('llm-test')).toHaveTextContent('« Двое идут на B » → « Deux vont sur B »');
    // A change of settings hides the old result.
    fireEvent.click(screen.getByRole('radio', { name: /Groq/ }));
    expect(screen.queryByTestId('llm-test')).toBeNull();
  });

  it('translates the user’s sentence, French into Russian by default', async () => {
    useLlm.setState({ native: true, status: status() });
    vi.spyOn(llm, 'configure').mockResolvedValue(status());
    vi.spyOn(llm, 'status').mockResolvedValue(status());
    const tr = vi.spyOn(llm, 'translateTest').mockResolvedValue({ ok: true, ms: 702, original: TRY_SENTENCE, translated: 'Моя команда не умеет играть.', error: null });
    render(<LlmCard />);
    await act(async () => fireEvent.click(screen.getByText('Traduire')));
    expect(tr).toHaveBeenCalledWith('Mon équipe ne sait pas jouer.', 'fr', 'ru');
    expect(await screen.findByTestId('llm-test')).toHaveTextContent('« Mon équipe ne sait pas jouer. » → « Моя команда не умеет играть. »');
  });

  it('explains a failed connection test', async () => {
    useLlm.setState({ native: true, status: status() });
    vi.spyOn(llm, 'configure').mockResolvedValue(status());
    vi.spyOn(llm, 'status').mockResolvedValue(status());
    vi.spyOn(llm, 'test').mockResolvedValue({ ok: false, ms: 210, original: 'Двое идут на B', translated: null, error: 'clé refusée par le fournisseur' });
    render(<LlmCard />);
    await act(async () => fireEvent.click(screen.getByText('Tester la connexion')));
    expect(await screen.findByTestId('llm-test')).toHaveTextContent('✗ clé refusée par le fournisseur');
    expect(screen.getByTestId('llm-test')).toHaveTextContent('Créez-en une nouvelle');
  });
});
