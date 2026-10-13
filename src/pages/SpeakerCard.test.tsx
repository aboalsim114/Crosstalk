import { beforeEach, describe, expect, it } from 'vitest';
import { act, fireEvent, render, screen } from '@testing-library/react';
import { useSpeakerStatus } from '../state/speaker';
import { useApp } from '../state/store';
import { SpeakerCard } from './SpeakerCard';

const app0 = useApp.getState();
beforeEach(() => {
  useApp.setState(app0, true);
  useSpeakerStatus.setState({ status: null });
});

describe('SpeakerCard', () => {
  it('turns the reading on and lets the user mark their own name', () => {
    render(<SpeakerCard />);
    expect(screen.getByTestId('speaker-state').textContent).toBe('Désactivé');
    fireEvent.click(screen.getByRole('switch', { name: /pseudo de celui qui parle/ }));
    expect(useApp.getState().speaker.enabled).toBe(true);
    act(() => {
      useSpeakerStatus.setState({ status: { enabled: true, state: 'watching', detail: null, ocrLang: 'fr-FR', known: ['Player One', 'Moi'], blank: false, covered: false } });
    });
    expect(screen.getByTestId('speaker-state').textContent).toBe('Actif');
    fireEvent.click(screen.getByText('Moi'));
    expect(useApp.getState().ownName).toBe('Moi');
  });

  it('moves the area read with the sliders', () => {
    render(<SpeakerCard />);
    fireEvent.change(screen.getByLabelText('Zone lue · haut'), { target: { value: '62' } });
    expect(useApp.getState().speaker.region.y).toBeCloseTo(0.62);
  });
});
