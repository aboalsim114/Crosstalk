import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { llm, LLM_DEFAULT_URL, type LlmStatus } from '../../shared/ipc';
import { LLM_DEFAULTS, useApp } from '../../app/state/store';
import { engineLabel, llmConfigFrom, llmStatsLine, llmSummary, providerChange, useLlm } from './llm';
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
    const c = llmConfigFrom({ llm: { enabled: true, provider: 'anthropic', baseUrl: ' https://api.anthropic.com/v1 ', model: ' claude-haiku-4-5 ' } });
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

describe('LLM card', () => {
  it('shows the switch off, the provider, address, model and the privacy notice', () => {
    render(<LlmCard />);
    expect(screen.getByText('TRADUCTION EN LIGNE · LLM')).toBeInTheDocument();
    expect(screen.getByTestId('llm-state')).toHaveTextContent('Désactivé · Opus-MT local seulement');
    expect(screen.getByRole('textbox', { name: 'Adresse de base' })).toHaveValue(LLM_DEFAULT_URL.openai);
    expect(screen.getByTestId('llm-privacy')).toHaveTextContent('jamais l’audio, jamais les pseudos des joueurs');
    // Outside the desktop app the key cannot be stored.
    expect(screen.getByLabelText('Clé API')).toBeDisabled();
  });

  it('switches provider, pre-fills its address and keeps typed settings in the app state', () => {
    render(<LlmCard />);
    fireEvent.click(screen.getByText('Anthropic'));
    expect(useApp.getState().llm.provider).toBe('anthropic');
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
});
