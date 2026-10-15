import { act, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { llm } from '../../shared/ipc';
import { useApp } from '../../app/state/store';
import { EngineSetup, engineSetupShown } from './EngineSetup';
import { useLlm } from './llm';

const app0 = useApp.getState();
const llm0 = useLlm.getState();
beforeEach(() => {
  useApp.setState({ ...app0, langSetupDone: true, engineSetupDone: false, page: 'dash' }, true);
  useLlm.setState(llm0, true);
});
afterEach(() => vi.restoreAllMocks());

describe('first launch: translation engine', () => {
  it('is asked once after the languages, never over an engine already connected', () => {
    expect(engineSetupShown({ langSetupDone: true, engineSetupDone: false, llm: { enabled: false } })).toBe(true);
    expect(engineSetupShown({ langSetupDone: false, engineSetupDone: false, llm: { enabled: false } })).toBe(false);
    expect(engineSetupShown({ langSetupDone: true, engineSetupDone: true, llm: { enabled: false } })).toBe(false);
    expect(engineSetupShown({ langSetupDone: true, engineSetupDone: false, llm: { enabled: true } })).toBe(false);
  });

  it('offers API Cloud, Ollama, LM Studio and later', () => {
    render(<EngineSetup />);
    expect(screen.getByRole('dialog', { name: 'Moteur de traduction' })).toBeInTheDocument();
    for (const id of ['cloud', 'ollama', 'lmstudio', 'later']) expect(screen.getByTestId(`engine-${id}`)).toBeInTheDocument();
    expect(screen.getByTestId('engine-later')).toHaveTextContent('la traduction restera indisponible');
  });

  it('"Configurer plus tard" leaves translation off and the app usable', async () => {
    const { rerender } = render(<EngineSetup />);
    await act(async () => fireEvent.click(screen.getByTestId('engine-later')));
    expect(useApp.getState()).toMatchObject({ engineSetupDone: true, page: 'dash' });
    expect(useApp.getState().llm.enabled).toBe(false);
    rerender(<EngineSetup />);
    expect(screen.queryByRole('dialog')).toBeNull();
  });

  it('API Cloud opens Traduction IA without connecting anything', async () => {
    render(<EngineSetup />);
    await act(async () => fireEvent.click(screen.getByTestId('engine-cloud')));
    expect(useApp.getState().page).toBe('audio');
    expect(useApp.getState().llm).toMatchObject({ service: 'openai', enabled: false, cloudConsent: false });
  });

  it('Ollama picks an installed model and downloads none', async () => {
    const detect = vi.spyOn(llm, 'localServers').mockResolvedValue([{ kind: 'ollama', baseUrl: 'http://localhost:11434/v1', running: true, models: ['translategemma:4b'] }]);
    render(<EngineSetup />);
    await act(async () => fireEvent.click(screen.getByTestId('engine-ollama')));
    expect(detect).toHaveBeenCalled();
    expect(useApp.getState().llm).toMatchObject({ service: 'ollama', baseUrl: 'http://localhost:11434/v1', model: 'translategemma:4b', enabled: false });
    expect(useApp.getState().page).toBe('audio');
  });

  it('LM Studio absent: no model is invented', async () => {
    vi.spyOn(llm, 'localServers').mockResolvedValue([{ kind: 'lmstudio', baseUrl: 'http://localhost:1234/v1', running: false, models: [] }]);
    render(<EngineSetup />);
    await act(async () => fireEvent.click(screen.getByTestId('engine-lmstudio')));
    expect(useApp.getState().llm).toMatchObject({ service: 'lmstudio', model: '', enabled: false });
  });
});
