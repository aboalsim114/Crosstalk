import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { llm, LLM_DEFAULT_URL, LLM_SERVICES, serviceOf, type LlmStatus } from '../../shared/ipc';
import { LLM_DEFAULTS, useApp } from '../../app/state/store';
import { engineLabel, llmConfigFrom, llmStatsLine, llmSummary, providerChange, serviceChange, testAdvice, useLlm } from './llm';
import { LlmCard } from './LlmCard';

// Simulated statuses: no real provider is called in these tests.
const status = (p: Partial<LlmStatus> = {}): LlmStatus => ({
  config: { enabled: true, provider: 'openai', baseUrl: LLM_DEFAULT_URL.openai, model: 'gpt-4o-mini' },
  hasKey: true,
  active: true,
  problem: null,
  keyStorage: 'Gestionnaire d’identifiants Windows',
  calls: 0,
  failures: 0,
  lastLatencyMs: null,
  lastError: null,
  ...p,
});

const app0 = useApp.getState();
const llm0 = useLlm.getState();
beforeEach(() => {
  useApp.setState(app0, true);
  useLlm.setState(llm0, true);
});
afterEach(() => vi.restoreAllMocks());

describe('online LLM settings', () => {
  it('is off by default and sends trimmed settings, never a key', () => {
    expect(LLM_DEFAULTS.enabled).toBe(false);
    const c = llmConfigFrom({ llm: { enabled: true, service: 'anthropic', provider: 'anthropic', baseUrl: ' https://api.anthropic.com/v1 ', model: ' claude-haiku-4-5 ' } });
    expect(c).toEqual({ enabled: true, provider: 'anthropic', baseUrl: 'https://api.anthropic.com/v1', model: 'claude-haiku-4-5' });
    expect(Object.keys(c)).not.toContain('key');
  });

  it('pre-fills the address of the provider unless the user typed one', () => {
    expect(providerChange('anthropic', { provider: 'openai', baseUrl: LLM_DEFAULT_URL.openai })).toEqual({ provider: 'anthropic', baseUrl: LLM_DEFAULT_URL.anthropic });
    expect(providerChange('anthropic', { provider: 'openai', baseUrl: '' }).baseUrl).toBe(LLM_DEFAULT_URL.anthropic);
    const ollama = 'http://localhost:11434/v1';
    expect(providerChange('anthropic', { provider: 'openai', baseUrl: ollama }).baseUrl).toBe(ollama);
  });

  it('summarises the state and the counters', () => {
    expect(llmSummary(null, false).value).toMatch(/^Désactivé/);
    expect(llmSummary(status(), true)).toEqual({ value: 'Actif · Opus-MT en secours', color: '#7fe0a8' });
    expect(llmSummary(status({ active: false, problem: 'clé API non enregistrée' }), true).value).toBe('Problème : clé API non enregistrée · Opus-MT utilisé');
    expect(llmStatsLine(status({ calls: 12, failures: 1, lastLatencyMs: 640 }))).toBe('12 appels · 1 échec · dernière latence 640 ms');
    expect(llmStatsLine(status({ calls: 1, failures: 0 }))).toBe('1 appel · 0 échec');
    expect(engineLabel('llm')).toBe('LLM en ligne');
    expect(engineLabel('opus-mt')).toBe('Opus-MT local');
    expect(engineLabel('lexicon')).toBe('lexique Counter-Strike');
    expect(engineLabel(null)).toBeNull();
  });
});

describe('ready-made services', () => {
  it('fills address and model, Ollama Cloud with a key, Ollama on this PC without one', () => {
    const cloud = serviceChange('ollama-cloud', LLM_DEFAULTS);
    expect(cloud).toEqual({ service: 'ollama-cloud', provider: 'openai', baseUrl: 'https://ollama.com/v1', model: 'gemma4:31b' });
    expect(LLM_SERVICES.find((s) => s.id === 'ollama-cloud')?.local).toBe(false);
    expect(serviceChange('anthropic', LLM_DEFAULTS)).toMatchObject({ provider: 'anthropic', baseUrl: 'https://api.anthropic.com/v1' });
    // Ollama on this PC: the model comes from the installed ones.
    expect(serviceChange('ollama', LLM_DEFAULTS, ['llama3.2:3b', 'qwen2.5:7b'])).toEqual({ service: 'ollama', provider: 'openai', baseUrl: 'http://localhost:11434/v1', model: 'llama3.2:3b' });
    expect(serviceChange('ollama', { ...LLM_DEFAULTS, model: 'qwen2.5:7b' }, ['llama3.2:3b', 'qwen2.5:7b']).model).toBe('qwen2.5:7b');
    expect(serviceChange('ollama', LLM_DEFAULTS).model).toBe('');
    expect(serviceChange('custom', LLM_DEFAULTS)).toEqual({ service: 'custom' });
    for (const s of LLM_SERVICES) {
      expect(s.baseUrl.startsWith('https://') || s.local).toBe(true);
      expect(serviceOf(s)).toBe(s.id);
    }
  });

  it('turns test errors into plain advice', () => {
    expect(testAdvice('clé refusée par le fournisseur', 'openai')).toMatch(/Créez-en une nouvelle/);
    expect(testAdvice('clé API non enregistrée', 'groq')).toMatch(/Collez votre clé/);
    expect(testAdvice('crédit épuisé chez le fournisseur (402)', 'openai')).toMatch(/crédit/);
    expect(testAdvice('quota ou limite de débit atteint (429)', 'openrouter')).toMatch(/quota/);
    expect(testAdvice('modèle ou adresse introuvable (404)', 'mistral')).toMatch(/Réglages avancés/);
    expect(testAdvice('réseau indisponible (connexion refusée)', 'ollama')).toMatch(/serveur local ne répond pas/);
    expect(testAdvice('réseau indisponible (dns)', 'openai')).toMatch(/connexion Internet/);
    expect(testAdvice('modèle non renseigné', 'ollama')).toMatch(/modèle installé/);
    expect(testAdvice('délai dépassé (6.0 s)', 'anthropic')).toMatch(/lent/);
  });
});

describe('LLM card', () => {
  it('shows the switch off, the services, the steps and the privacy notice', () => {
    render(<LlmCard />);
    expect(screen.getByText('TRADUCTION EN LIGNE · LLM')).toBeInTheDocument();
    expect(screen.getByTestId('llm-state')).toHaveTextContent('Désactivé · Opus-MT local seulement');
    expect(screen.getByRole('radio', { name: /OpenAI/ })).toHaveAttribute('aria-checked', 'true');
    for (const name of ['Ollama Cloud', 'Ollama sur ce PC', 'Mistral AI', 'Autre']) expect(screen.getByRole('radio', { name: new RegExp(name) })).toBeInTheDocument();
    expect(screen.getByText('Obtenir ma clé OpenAI ↗')).toBeInTheDocument();
    // Address and model stay out of the way.
    expect(screen.queryByRole('textbox', { name: 'Adresse de base' })).toBeNull();
    fireEvent.click(screen.getByText(/Réglages avancés/));
    expect(screen.getByRole('textbox', { name: 'Adresse de base' })).toHaveValue(LLM_DEFAULT_URL.openai);
    expect(screen.getByRole('textbox', { name: 'Modèle' })).toHaveValue('gpt-4o-mini');
    expect(screen.getByTestId('llm-privacy')).toHaveTextContent('jamais l’audio, jamais les pseudos des joueurs');
    // Outside the desktop app the key cannot be stored.
    expect(screen.getByLabelText('Clé API')).toBeDisabled();
  });

  it('switches service, fills its settings and keeps typed ones in the app state', () => {
    render(<LlmCard />);
    fireEvent.click(screen.getByRole('radio', { name: /Anthropic/ }));
    expect(useApp.getState().llm).toMatchObject({ service: 'anthropic', provider: 'anthropic', model: 'claude-haiku-5-5' });
    fireEvent.click(screen.getByText(/Réglages avancés/));
    expect(screen.getByRole('textbox', { name: 'Adresse de base' })).toHaveValue(LLM_DEFAULT_URL.anthropic);
    fireEvent.change(screen.getByRole('textbox', { name: 'Modèle' }), { target: { value: 'claude-haiku-4-5' } });
    expect(useApp.getState().llm.model).toBe('claude-haiku-4-5');
    fireEvent.click(screen.getByRole('switch', { name: 'Traduire avec un LLM en ligne' }));
    expect(useApp.getState().llm.enabled).toBe(true);
  });

  it('sends the key once, empties the field and never shows it again', async () => {
    useLlm.setState({ native: true, status: status({ hasKey: false, active: false, problem: 'clé API non enregistrée' }) });
    useApp.setState({ llm: { ...LLM_DEFAULTS, enabled: true, model: 'gpt-4o-mini' } });
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

  it('shows why a key was refused and lets the user clear a stored one', async () => {
    useLlm.setState({ native: true, status: status() });
    useApp.setState({ llm: { ...LLM_DEFAULTS, enabled: true, model: 'gpt-4o-mini' } });
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

  it('shows calls, failures, last latency and last error while enabled', () => {
    useLlm.setState({ native: true, status: status({ calls: 7, failures: 2, lastLatencyMs: 812, lastError: 'quota ou limite de débit atteint (429)' }) });
    useApp.setState({ llm: { ...LLM_DEFAULTS, enabled: true, model: 'gpt-4o-mini' } });
    render(<LlmCard />);
    expect(screen.getByTestId('llm-state')).toHaveTextContent('Actif · Opus-MT en secours');
    expect(screen.getByTestId('llm-stats')).toHaveTextContent('7 appels · 2 échecs · dernière latence 812 ms');
    expect(screen.getByTestId('llm-last-error')).toHaveTextContent('Dernière erreur : quota ou limite de débit atteint (429)');
  });

  it('opens the key page of the chosen service, Ollama Cloud included', async () => {
    useLlm.setState({ native: true, status: status({ hasKey: false }) });
    const open = vi.spyOn(llm, 'openKeyPage').mockResolvedValue();
    render(<LlmCard />);
    fireEvent.click(screen.getByRole('radio', { name: /Ollama Cloud/ }));
    expect(useApp.getState().llm.baseUrl).toBe('https://ollama.com/v1');
    fireEvent.click(screen.getByText('Obtenir ma clé Ollama Cloud ↗'));
    expect(open).toHaveBeenCalledWith('ollama-cloud');
    expect(screen.getByLabelText('Clé API')).toBeEnabled();
  });

  it('detects Ollama on this PC, picks an installed model and asks for no key', async () => {
    useLlm.setState({ native: true, status: status({ hasKey: false }) });
    const detect = vi.spyOn(llm, 'ollamaLocal').mockResolvedValue({ running: true, models: ['llama3.2:3b', 'qwen2.5:7b'] });
    vi.spyOn(llm, 'configure').mockResolvedValue(null);
    render(<LlmCard />);
    await act(async () => fireEvent.click(screen.getByRole('radio', { name: /Ollama sur ce PC/ })));
    expect(detect).toHaveBeenCalled();
    expect(await screen.findByText('Ollama détecté · 2 modèles installés')).toBeInTheDocument();
    expect(useApp.getState().llm).toMatchObject({ service: 'ollama', baseUrl: 'http://localhost:11434/v1', model: 'llama3.2:3b' });
    fireEvent.change(screen.getByLabelText('Modèle installé'), { target: { value: 'qwen2.5:7b' } });
    expect(useApp.getState().llm.model).toBe('qwen2.5:7b');
    expect(screen.queryByLabelText('Clé API')).toBeNull();
  });

  it('offers to install Ollama when it is not running, then checks again', async () => {
    useLlm.setState({ native: true, status: status({ hasKey: false }) });
    const detect = vi.spyOn(llm, 'ollamaLocal').mockResolvedValue({ running: false, models: [] });
    const open = vi.spyOn(llm, 'openKeyPage').mockResolvedValue();
    render(<LlmCard />);
    await act(async () => fireEvent.click(screen.getByRole('radio', { name: /Ollama sur ce PC/ })));
    expect(await screen.findByText('Ollama n’est pas détecté sur ce PC.')).toBeInTheDocument();
    expect(screen.getByTestId('llm-ollama')).toHaveTextContent('ollama pull qwen2.5:7b');
    fireEvent.click(screen.getByText('Télécharger Ollama ↗'));
    expect(open).toHaveBeenCalledWith('ollama-download');
    detect.mockResolvedValue({ running: true, models: [] });
    await act(async () => fireEvent.click(screen.getByText('Vérifier à nouveau')));
    expect(await screen.findByText('Ollama détecté, mais aucun modèle n’est installé.')).toBeInTheDocument();
  });

  it('tests the connection with the settings on screen and shows the sample translation', async () => {
    useLlm.setState({ native: true, status: status() });
    const configure = vi.spyOn(llm, 'configure').mockResolvedValue(status());
    vi.spyOn(llm, 'status').mockResolvedValue(status());
    vi.spyOn(llm, 'test').mockResolvedValue({ ok: true, ms: 640, original: 'Двое идут на B', translated: 'Deux vont sur B', error: null });
    render(<LlmCard />);
    await act(async () => fireEvent.click(screen.getByText('Tester la connexion')));
    expect(configure).toHaveBeenCalledWith(llmConfigFrom(useApp.getState()));
    expect(await screen.findByTestId('llm-test')).toHaveTextContent('✓ Connexion réussie · 640 ms');
    expect(screen.getByTestId('llm-test')).toHaveTextContent('« Двое идут на B » → « Deux vont sur B »');
    // A change of settings hides the old result.
    fireEvent.click(screen.getByRole('radio', { name: /Groq/ }));
    expect(screen.queryByTestId('llm-test')).toBeNull();
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
