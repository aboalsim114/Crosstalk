import { beforeEach, describe, expect, it, vi } from 'vitest';
import { act, fireEvent, render, screen, waitFor } from '@testing-library/react';
import type { TranslationResult, TranslationStatus } from '../backend/translation';
import { useTranslation } from '../state/translation';
import { useApp } from '../state/store';
import { TranslationCard } from './TranslationCard';

const api = vi.hoisted(() => ({ status: vi.fn(), configure: vi.fn(), saveKey: vi.fn(), deleteKey: vi.fn(), test: vi.fn() }));
vi.mock('../backend/translation', async (original) => ({
  ...(await original<typeof import('../backend/translation')>()), translation: api,
}));

const idle: TranslationStatus = { enabled: false, plan: 'free', keyConfigured: false, supported: true };
const ready = { ...idle, enabled: true, keyConfigured: true };
const answer: TranslationResult = {
  original: 'Two players on B, wait for my flash.', translated: 'Deux joueurs sur B, attendez ma flash.',
  source: 'en', target: 'fr', status: 'translated', reason: null, cached: false, latencyMs: 320,
};
const initialApp = useApp.getState();
beforeEach(() => {
  vi.resetAllMocks();
  useApp.setState(initialApp, true);
  useTranslation.setState({ native: true, status: null }, true);
  api.status.mockResolvedValue(idle);
  api.configure.mockImplementation(({ enabled, plan }) => Promise.resolve({ ...ready, enabled, plan }));
  api.saveKey.mockResolvedValue({ ...idle, keyConfigured: true });
  api.deleteKey.mockResolvedValue(idle);
  api.test.mockResolvedValue(answer);
});

describe('manual DeepL translation configuration', () => {
  it('does not let a late initial refresh overwrite a newer disable', async () => {
    useTranslation.setState({ status: ready });
    let complete!: (value: TranslationStatus) => void;
    api.status.mockImplementation(() => new Promise<TranslationStatus>((resolve) => { complete = resolve; }));
    render(<TranslationCard />);
    fireEvent.click(screen.getByRole('checkbox', { name: 'Activer les tests DeepL' }));
    await waitFor(() => expect(screen.getByRole('checkbox', { name: 'Activer les tests DeepL' })).not.toBeChecked());
    await act(async () => complete(ready));
    expect(screen.getByRole('checkbox', { name: 'Activer les tests DeepL' })).not.toBeChecked();
  });

  it('orders disabling after a pending test configuration and sends no phrase', async () => {
    api.status.mockResolvedValue(ready);
    let complete!: (value: TranslationStatus) => void;
    api.configure.mockImplementationOnce(() => new Promise<TranslationStatus>((resolve) => { complete = resolve; }));
    render(<TranslationCard />);
    await waitFor(() => expect(screen.getByRole('button', { name: 'Tester la traduction' })).toBeEnabled());
    fireEvent.click(screen.getByRole('button', { name: 'Tester la traduction' }));
    await waitFor(() => expect(api.configure).toHaveBeenCalledTimes(1));
    fireEvent.click(screen.getByRole('checkbox', { name: 'Activer les tests DeepL' }));
    expect(api.configure).toHaveBeenCalledTimes(1);
    await act(async () => complete(ready));
    await waitFor(() => expect(api.configure).toHaveBeenCalledTimes(2));
    expect(api.configure.mock.calls[1][0].enabled).toBe(false);
    expect(api.test).not.toHaveBeenCalled();
    expect(screen.getByRole('checkbox', { name: 'Activer les tests DeepL' })).not.toBeChecked();
  });

  it('does not activate or send a phrase just because the page opens', async () => {
    render(<TranslationCard />);
    await waitFor(() => expect(screen.getByRole('combobox', { name: 'Compte DeepL API' })).toBeEnabled());
    expect(screen.getByRole('checkbox', { name: 'Activer les tests DeepL' })).not.toBeChecked();
    expect(screen.getByRole('checkbox', { name: 'Activer les tests DeepL' })).toBeDisabled();
    expect(screen.getByRole('button', { name: 'Tester la traduction' })).toBeDisabled();
    expect(api.configure).not.toHaveBeenCalled();
    expect(api.test).not.toHaveBeenCalled();
  });

  it('clears the password field after saving, without enabling transmission', async () => {
    render(<TranslationCard />);
    const input = screen.getByLabelText('Clé API DeepL');
    await waitFor(() => expect(input).toBeEnabled());
    expect(input).toHaveAttribute('type', 'password');
    fireEvent.change(input, { target: { value: 'synthetic-test-key-only' } });
    fireEvent.click(screen.getByRole('button', { name: 'Enregistrer la clé' }));
    await waitFor(() => expect(api.saveKey).toHaveBeenCalledWith('free', 'synthetic-test-key-only'));
    expect(input).toHaveValue('');
    expect(screen.queryByText('synthetic-test-key-only')).not.toBeInTheDocument();
    expect(screen.getByRole('checkbox', { name: 'Activer les tests DeepL' })).not.toBeChecked();
    expect(api.test).not.toHaveBeenCalled();
  });

  it('uses the current glossary and marks a provider result as a French translation', async () => {
    api.status.mockResolvedValue(ready);
    useApp.setState({ gloss: [['smoke', 'Conserver', '—'], ['site', 'Traduire', 'site']] });
    render(<TranslationCard />);
    const button = screen.getByRole('button', { name: 'Tester la traduction' });
    await waitFor(() => expect(button).toBeEnabled());
    fireEvent.click(button);
    expect(await screen.findByText(answer.translated!)).toBeInTheDocument();
    expect(api.configure).toHaveBeenCalledWith({ enabled: true, plan: 'free', glossary: [{ source: 'smoke', target: null }, { source: 'site', target: 'site' }] });
    expect(api.test).toHaveBeenCalledWith(answer.original, 'en');
    expect(screen.getByText('TRADUCTION FRANÇAISE')).toBeInTheDocument();
    expect(screen.getByText('320 ms')).toBeInTheDocument();
  });

  it('keeps fallback output visibly original rather than French', async () => {
    api.status.mockResolvedValue(ready);
    api.test.mockResolvedValue({ ...answer, translated: null, status: 'original', reason: 'Moteur indisponible.' });
    render(<TranslationCard />);
    await waitFor(() => expect(screen.getByRole('button', { name: 'Tester la traduction' })).toBeEnabled());
    fireEvent.click(screen.getByRole('button', { name: 'Tester la traduction' }));
    expect(await screen.findByText('ORIGINAL')).toBeInTheDocument();
    expect(screen.queryByText('TRADUCTION FRANÇAISE')).not.toBeInTheDocument();
    expect(screen.getByText('Moteur indisponible.')).toBeInTheDocument();
  });

  it('discards a late result when authorization is disabled during the test', async () => {
    api.status.mockResolvedValue(ready);
    let complete!: (value: TranslationResult) => void;
    api.test.mockImplementation(() => new Promise<TranslationResult>((resolve) => { complete = resolve; }));
    render(<TranslationCard />);
    await waitFor(() => expect(screen.getByRole('button', { name: 'Tester la traduction' })).toBeEnabled());
    fireEvent.click(screen.getByRole('button', { name: 'Tester la traduction' }));
    await waitFor(() => expect(api.test).toHaveBeenCalledTimes(1));
    fireEvent.click(screen.getByRole('checkbox', { name: 'Activer les tests DeepL' }));
    await waitFor(() => expect(screen.getByRole('checkbox', { name: 'Activer les tests DeepL' })).not.toBeChecked());
    await act(async () => complete(answer));
    expect(screen.queryByText(answer.translated!)).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Tester la traduction' })).toBeDisabled();
  });

  it('permits a French original without credentials and labels it as original', async () => {
    api.test.mockResolvedValue({ ...answer, source: 'fr', original: 'Deux joueurs sur B.', translated: null, status: 'original', reason: 'Déjà en français.' });
    render(<TranslationCard />);
    await waitFor(() => expect(screen.getByRole('combobox', { name: 'Compte DeepL API' })).toBeEnabled());
    fireEvent.change(screen.getByRole('combobox', { name: 'Langue du texte' }), { target: { value: 'fr' } });
    fireEvent.change(screen.getByLabelText('Phrase à tester'), { target: { value: 'Deux joueurs sur B.' } });
    const button = screen.getByRole('button', { name: 'Afficher l’original français' });
    expect(button).toBeEnabled();
    fireEvent.click(button);
    expect(await screen.findByText('Déjà en français.')).toBeInTheDocument();
    expect(screen.getByText('ORIGINAL')).toBeInTheDocument();
    expect(api.test).toHaveBeenCalledWith('Deux joueurs sur B.', 'fr');
    expect(api.configure).toHaveBeenCalledWith(expect.objectContaining({ enabled: false }));
  });

  it('never echoes a rejected credential exception containing sensitive data', async () => {
    api.saveKey.mockRejectedValue(new Error('provider echoed synthetic-sensitive-key'));
    render(<TranslationCard />);
    await waitFor(() => expect(screen.getByLabelText('Clé API DeepL')).toBeEnabled());
    fireEvent.change(screen.getByLabelText('Clé API DeepL'), { target: { value: 'synthetic-sensitive-key' } });
    fireEvent.click(screen.getByRole('button', { name: 'Enregistrer la clé' }));
    expect(await screen.findByRole('alert')).toHaveTextContent('Modification impossible.');
    expect(screen.queryByText(/provider echoed/)).not.toBeInTheDocument();
    expect(screen.getByLabelText('Clé API DeepL')).toHaveValue('');
  });
});
