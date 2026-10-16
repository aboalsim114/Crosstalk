import { act, fireEvent, render, screen } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { SetupProgress } from './api';

// The Rust side, driven by the test.
const h = vi.hoisted(() => ({
  emit: (_: unknown) => {},
  calls: [] as unknown[][],
}));
vi.mock('./api', async (orig) => {
  const real = await orig<typeof import('./api')>();
  const rec =
    (name: string, value?: unknown) =>
    (...args: unknown[]) => {
      h.calls.push([name, ...args]);
      return Promise.resolve(value);
    };
  return {
    ...real,
    native: true,
    setup: {
      info: rec('info', { version: '0.2.0', defaultDir: 'C:\\Users\\a\\AppData\\Local\\Crosstalk', appBytes: 26_000_000, modelBytes: 776_000_000, installed: null }),
      freeSpace: rec('freeSpace', 50_000_000_000),
      pickFolder: rec('pickFolder', 'D:\\Jeux\\Crosstalk'),
      install: rec('install'),
      cancel: rec('cancel'),
      launch: rec('launch'),
      setAutostart: rec('setAutostart'),
      minimize: rec('minimize'),
      quit: rec('quit'),
      onProgress: (cb: (p: SetupProgress) => void) => {
        h.emit = cb as (p: unknown) => void;
        return Promise.resolve(() => {});
      },
    },
  };
});

import { Installer } from './Installer';

const progress = (p: Partial<SetupProgress>): SetupProgress => ({ phase: 1, pct: 30, detail: '', done: false, error: null, cancelled: false, ...p });

beforeEach(() => {
  h.calls = [];
  // Reduced motion: screens switch without the (jsdom-less) Web Animations.
  window.matchMedia = ((q: string) => ({ matches: q.includes('reduce'), media: q, addEventListener() {}, removeEventListener() {} })) as unknown as typeof window.matchMedia;
  Element.prototype.animate = vi.fn(() => ({ finished: Promise.resolve() })) as unknown as typeof Element.prototype.animate;
});

describe('Installer', () => {
  it('shows the real version, sizes and folder, then installs with the chosen options', async () => {
    render(<Installer />);
    expect(await screen.findByText('VERSION 0.2.0')).toBeInTheDocument();
    fireEvent.click(screen.getByText("INITIALISER L'INSTALLATION"));
    expect(await screen.findByTestId('setup-required')).toHaveTextContent('802 Mo');
    expect(screen.getByText('Disponible : 50 Go')).toBeInTheDocument();
    expect(screen.queryByText(/DÉMONSTRATION|SIMULATION/)).toBeNull();

    fireEvent.click(screen.getByText('Parcourir…'));
    expect(await screen.findByText('D:\\Jeux\\Crosstalk')).toBeInTheDocument();
    fireEvent.click(screen.getByText('Créer un raccourci sur le bureau'));
    fireEvent.click(screen.getByText('INSTALLER CROSSTALK'));
    expect(h.calls).toContainEqual(['install', { dir: 'D:\\Jeux\\Crosstalk', desktop: false }]);

    act(() => h.emit(progress({ detail: 'Reconnaissance vocale (Parakeet TDT 0.6B v3) · 120 / 671 Mo' })));
    expect(await screen.findByText('Reconnaissance vocale (Parakeet TDT 0.6B v3) · 120 / 671 Mo')).toBeInTheDocument();
    expect(screen.getByText('EN COURS')).toBeInTheDocument();
  });

  it('reports an error with a retry instead of pretending', async () => {
    render(<Installer startScreen={1} />);
    fireEvent.click(await screen.findByText('INSTALLER CROSSTALK'));
    act(() => h.emit(progress({ pct: 0, error: 'Téléchargement des modèles interrompu : connexion impossible.' })));
    expect(await screen.findByText('L’installation n’a pas abouti.')).toBeInTheDocument();
    expect(screen.getByText('ERREUR')).toBeInTheDocument();
    fireEvent.click(screen.getByText('RÉESSAYER'));
    expect(await screen.findByText('INSTALLER CROSSTALK')).toBeInTheDocument();
  });

  it('asks before quitting, then cancels and closes', async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    render(<Installer startScreen={1} />);
    fireEvent.click(screen.getByLabelText('Fermer'));
    fireEvent.click(await screen.findByText('QUITTER'));
    expect(h.calls.some((c) => c[0] === 'cancel')).toBe(true);
    await act(async () => vi.advanceTimersByTime(2000));
    expect(h.calls.some((c) => c[0] === 'quit')).toBe(true);
    vi.useRealTimers();
  });

  it('launches Crosstalk with the autostart choice', async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    render(<Installer startScreen={3} />);
    await act(async () => {}); // setup_info answered
    fireEvent.click(screen.getByRole('switch', { checked: false }));
    fireEvent.click(screen.getByText('LANCER CROSSTALK'));
    await act(async () => {}); // autostart, then launch
    await act(async () => vi.advanceTimersByTime(2000));
    const dir = 'C:\\Users\\a\\AppData\\Local\\Crosstalk';
    expect(h.calls).toContainEqual(['setAutostart', dir, true]);
    expect(h.calls).toContainEqual(['launch', dir]);
    expect(h.calls.some((c) => c[0] === 'quit')).toBe(true);
    vi.useRealTimers();
  });
});
