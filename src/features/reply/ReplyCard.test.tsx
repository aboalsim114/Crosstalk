import { act, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { reply } from '../../shared/ipc';
import { ReplyCard } from './ReplyCard';

afterEach(() => {
  vi.useRealTimers();
  vi.restoreAllMocks();
});

describe('reply card', () => {
  it('says free text needs the engine, with no model to download', async () => {
    vi.useFakeTimers();
    vi.spyOn(reply, 'models').mockResolvedValue([{ source: 'fr', target: 'ru', ready: false, problem: 'aucun moteur de traduction connecté (Audio & IA › Traduction IA)' }]);
    render(<ReplyCard />);
    await act(async () => vi.advanceTimersByTime(700));
    expect(screen.getByTestId('reply-unavailable')).toHaveTextContent('Texte libre indisponible : aucun moteur de traduction connecté');
    expect(screen.getByTestId('reply-unavailable')).toHaveTextContent('phrases CS toutes prêtes fonctionnent toujours');
    expect(screen.queryByText('Télécharger')).toBeNull();
  });

  it('shows no warning once the engine is connected', async () => {
    vi.useFakeTimers();
    vi.spyOn(reply, 'models').mockResolvedValue([{ source: 'fr', target: 'ru', ready: true, problem: null }]);
    render(<ReplyCard />);
    await act(async () => vi.advanceTimersByTime(700));
    expect(screen.queryByTestId('reply-unavailable')).toBeNull();
    expect(screen.getByTestId('reply-card')).toHaveTextContent('Traduction IA');
  });
});
