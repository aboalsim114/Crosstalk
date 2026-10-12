import { beforeEach, describe, expect, it } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';
import { IDLE_CAPTURE } from '../backend';
import { useAudio } from '../state/audio';
import { useSpeech } from '../state/speech';
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
    expect(screen.getByText('OUTIL')).toBeInTheDocument();
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

describe('speech recognition cards in the desktop app', () => {
  const speech0 = useSpeech.getState();
  beforeEach(() => useSpeech.setState(speech0, true));

  it('lists the real model files and the engine state', () => {
    useSpeech.setState({
      native: true,
      status: {
        state: 'missing',
        config: { model: 'base', languages: ['ru', 'en'], vadThreshold: 0.55, glossary: [] },
        modelPath: null,
        vadReady: true,
        modelsDir: 'C:\\Users\\x\\AppData\\Local\\Crosstalk\\models',
        error: null,
        loadMs: null,
        backend: 'CPU · AVX2',
        threads: 4,
      },
      models: [
        { id: 'tiny', file: 'ggml-tiny.bin', sizeMb: 75, note: 'Très rapide', installed: true, url: '' },
        { id: 'base', file: 'ggml-base.bin', sizeMb: 142, note: 'Défaut', installed: false, url: '' },
      ],
    });
    useApp.setState({ model: 'base' });
    render(<AudioPage />);
    expect(screen.getByTestId('engine-state')).toHaveTextContent('Modèle absent');
    expect(screen.getByText('Sélectionné · téléchargement')).toBeInTheDocument();
    expect(screen.getByText('Installé')).toBeInTheDocument();
    expect(screen.getByText('Rechercher les modèles')).toBeInTheDocument();
    expect(screen.getByText(/CPU · AVX2 · SANS GPU/)).toBeInTheDocument();
    // Picking a model selects it; a missing one is then downloaded by the app.
    fireEvent.click(screen.getByText('whisper-tiny'));
    expect(useApp.getState().model).toBe('tiny');
    fireEvent.click(screen.getByText('whisper-base'));
    expect(useApp.getState().model).toBe('base');
  });

  it('shows the real VAD activity and rejection counters', () => {
    useSpeech.setState({
      native: true,
      activity: { speaking: true, transcribing: false },
      stats: { segments: 6, transcribed: 2, rejected: { transient: 4 }, droppedBlocks: 0, latencyP50Ms: 1100, latencyP95Ms: 1900, lastRtf: 0.4, cpuPercent: 9 },
    });
    render(<AudioPage />);
    expect(screen.getByTestId('vad-activity')).toHaveTextContent('● PAROLE');
    expect(screen.getByTestId('speech-stats')).toHaveTextContent('2 transcrits · 4 rejetés (tir / explosion 4)');
    expect(screen.getByTestId('asr-metrics')).toHaveTextContent('Latence p50 1,1 s · p95 1,9 s · RTF 0,40 · CPU 9 % de la machine');
  });
});

describe('local translation card', () => {
  it('shows the model pairs in the desktop app and no API key field', async () => {
    const { useTranslation } = await import('../state/translation');
    useTranslation.setState({
      native: true,
      status: {
        config: { enabled: true, glossary: [] },
        pairs: [
          { source: 'ru', repo: 'Helsinki-NLP/opus-mt-ru-fr', state: 'ready', error: null, loadMs: 1200 },
          { source: 'en', repo: 'Helsinki-NLP/opus-mt-en-fr', state: 'missing', error: null, loadMs: null },
        ],
        modelsDir: 'C:\\m',
        queued: 0,
      },
      times: [310, 420],
    });
    render(<AudioPage />);
    expect(screen.getByTestId('pair-ru')).toHaveTextContent('prêt · chargé en 1200 ms');
    expect(screen.getByTestId('pair-en')).toHaveTextContent('modèle absent');
    expect(screen.getByTestId('translation-metrics')).toHaveTextContent('Traduction p50 420 ms');
    expect(screen.getByTestId('translation-card')).toHaveTextContent('Aucune clé API');
    expect(screen.queryByText(/DeepL|Azure|cloud/i)).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole('switch', { name: 'Traduire les voix en français' }));
    expect(useApp.getState().tg.translate).toBe(false);
  });
});
