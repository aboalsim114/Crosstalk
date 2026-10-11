import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act, fireEvent, render, screen } from '@testing-library/react';
import App, { handleShortcut } from './App';
import { FALLBACK_INFO, getAppInfo, isTauri } from './backend';
import { useApp } from './state/store';

const initial = useApp.getState();

beforeEach(() => {
  vi.useFakeTimers();
  useApp.setState(initial, true);
});
afterEach(() => {
  useApp.getState().emergency();
  vi.useRealTimers();
});

describe('App', () => {
  it('opens on the cinematic intro', () => {
    render(<App />);
    expect(screen.getByTestId('intro')).toBeInTheDocument();
    expect(screen.getByText('en français.')).toBeInTheDocument();
  });

  it('enters and starts a session from the intro', () => {
    render(<App />);
    fireEvent.click(screen.getByText('Entrer et démarrer la session'));
    act(() => vi.advanceTimersByTime(1000));
    expect(screen.queryByTestId('intro')).not.toBeInTheDocument();
    act(() => vi.advanceTimersByTime(1400));
    expect(screen.getByTestId('session-badge')).toHaveTextContent('EN ÉCOUTE');
  });

  it('navigates between the seven chapters', () => {
    render(<App />);
    fireEvent.click(screen.getByText('Entrer'));
    for (const [label, chapter] of [
      ['Audio & IA', '03'],
      ['Diagnostic', '06'],
      ['À propos', '07'],
    ]) {
      fireEvent.click(screen.getAllByText(label)[0]);
      expect(screen.getByText(`CHAPITRE ${chapter} / 07`)).toBeInTheDocument();
    }
    expect(screen.getByText('Injecter une DLL dans cs2.exe')).toBeInTheDocument();
  });

  it('adds a glossary term', () => {
    useApp.setState({ entered: true, showIntro: false, page: 'lang' });
    render(<App />);
    fireEvent.change(screen.getByPlaceholderText(/Ajouter un terme/), { target: { value: 'boost' } });
    fireEvent.click(screen.getByText('Ajouter'));
    expect(screen.getByTestId('gloss-count')).toHaveTextContent('14 termes');
  });
});

describe('keyboard shortcuts', () => {
  const key = (k: string, mods = true) => handleShortcut({ key: k, ctrlKey: mods, altKey: mods, preventDefault: () => {} });

  it('Ctrl+Alt+F8 toggles the overlay', () => {
    key('F8');
    expect(useApp.getState().overlayOn).toBe(false);
    key('F8', false);
    expect(useApp.getState().overlayOn).toBe(false);
  });

  it('Ctrl+Alt+F10 is the emergency stop', () => {
    useApp.getState().toggleSession();
    key('F10');
    expect(useApp.getState().session).toBe('STOPPED');
  });
});

describe('backend bridge', () => {
  it('falls back to static info outside Tauri', async () => {
    expect(isTauri).toBe(false);
    await expect(getAppInfo()).resolves.toEqual(FALLBACK_INFO);
  });
});
