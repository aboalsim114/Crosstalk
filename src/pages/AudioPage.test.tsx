import { beforeEach, describe, expect, it } from 'vitest';
import { render, screen } from '@testing-library/react';
import { IDLE_CAPTURE } from '../backend';
import { useAudio } from '../state/audio';
import { useApp } from '../state/store';
import { AudioPage } from './AudioPage';

const audio0 = useAudio.getState();
const app0 = useApp.getState();
beforeEach(() => {
  useAudio.setState(audio0, true);
  useApp.setState(app0, true);
});

describe('Audio & IA in the desktop app', () => {
  it('shows the real level and source of a running capture', () => {
    useAudio.setState({
      native: true,
      cs2: { running: true, pid: 77 },
      status: { ...IDLE_CAPTURE, state: 'capturing', mode: 'auto', source: { kind: 'process', pid: 77, processName: 'cs2.exe' }, format: { sampleRate: 48000, channels: 2, sampleFormat: 'f32' } },
      level: { rms: 0.1, peak: 0.3, rmsDb: -20, peakDb: -10.5 },
    });
    render(<AudioPage />);
    expect(screen.getByText('CS2 · DÉTECTÉ')).toBeInTheDocument();
    expect(screen.getByText(/CS2 détecté \(PID 77\)/)).toBeInTheDocument();
    expect(screen.getByTestId('level-meter')).toHaveTextContent('−20,0 dBFS rms · −10,5 dBFS crête');
    expect(screen.getByRole('meter')).toHaveAttribute('aria-valuenow', '0.67');
    expect(screen.getByTestId('capture-state')).toHaveTextContent('Process loopback');
    expect(screen.getByText('À VENIR')).toBeInTheDocument();
    expect(screen.getByText('Tester la capture 10 s')).toBeInTheDocument();
  });

  it('lists output devices in device mode and warns about the whole-device capture', () => {
    useApp.setState({ capture: 'device' });
    useAudio.setState({ native: true, devices: [{ id: 'a', name: 'Casque USB', isDefault: true }] });
    render(<AudioPage />);
    expect(screen.getByRole('combobox', { name: 'Périphérique de sortie' })).toHaveTextContent('Casque USB (par défaut)');
    expect(screen.getByText(/capte tout le son du périphérique/)).toBeInTheDocument();
    expect(screen.getByTestId('level-meter')).toHaveTextContent('aucune capture');
  });

  it('keeps the simulated UI outside the desktop app', () => {
    render(<AudioPage />);
    expect(screen.queryByTestId('level-meter')).not.toBeInTheDocument();
    expect(screen.getByText('Test audio 10 s')).toBeInTheDocument();
  });
});
