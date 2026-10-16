import { beforeEach, describe, expect, it, vi } from 'vitest';
import { act, cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { IDLE_CAPTURE } from '../../shared/ipc';
import { useAudio } from './audio';
import { useSpeech } from '../speech/speech';
import { useApp } from '../../app/state/store';
import { AudioPage, focusNote } from './AudioPage';

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

  it('shows the recognition model, its state, languages and licence', () => {
    useSpeech.setState({
      native: true,
      status: {
        state: 'missing',
        config: { model: 'parakeet-tdt-0.6b-v3', languages: ['ru', 'en', 'fr'], vadThreshold: 0.55 },
        modelPath: null,
        vadReady: true,
        modelsDir: 'C:\\Users\\x\\AppData\\Local\\Crosstalk\\models',
        error: null,
        loadMs: null,
        backend: 'CPU · ONNX Runtime',
        threads: 4,
        modelLabel: 'Parakeet TDT 0.6B v3',
      },
      models: [{ id: 'parakeet-tdt-0.6b-v3', label: 'Parakeet TDT 0.6B v3', sizeMb: 670, note: 'Russe, anglais, français', license: 'CC-BY-4.0 (NVIDIA)', installed: false }],
    });
    render(<AudioPage />);
    expect(screen.getByTestId('engine-state')).toHaveTextContent('Modèle absent');
    expect(screen.getByTestId('engine-state')).toHaveTextContent('Parakeet TDT 0.6B v3 non installé');
    expect(screen.getByTestId('engine-langs')).toHaveTextContent('Langues écoutées : russe, anglais, français');
    expect(screen.queryByTestId('unsupported-langs')).toBeNull();
    expect(screen.getByText('Sélectionné · téléchargement')).toBeInTheDocument();
    expect(screen.getByText(/CC-BY-4.0/)).toBeInTheDocument();
    expect(screen.getByText('Rechercher les modèles')).toBeInTheDocument();
    // Nothing about Whisper or a CPU / GPU choice any more.
    expect(screen.queryByText(/whisper/i)).toBeNull();
    expect(screen.queryByText('GPU')).toBeNull();
  });

  it('says when the model is loaded, with its backend and load time', () => {
    useSpeech.setState({
      native: true,
      status: {
        state: 'ready',
        config: { model: 'parakeet-tdt-0.6b-v3', languages: ['ru', 'en'], vadThreshold: 0.55 },
        modelPath: 'C:\\models\\parakeet-tdt-0.6b-v3-int8',
        vadReady: true,
        modelsDir: 'C:\\models',
        error: null,
        loadMs: 6100,
        backend: 'CPU · ONNX Runtime',
        threads: 4,
        modelLabel: 'Parakeet TDT 0.6B v3',
      },
      models: [{ id: 'parakeet-tdt-0.6b-v3', label: 'Parakeet TDT 0.6B v3', sizeMb: 670, note: '', license: 'CC-BY-4.0 (NVIDIA)', installed: true }],
    });
    render(<AudioPage />);
    expect(screen.getByTestId('engine-state')).toHaveTextContent('Parakeet TDT 0.6B v3');
    expect(screen.getByTestId('engine-state')).toHaveTextContent('chargé en 6,1 s');
    // Turkish is not among the model's languages: said plainly.
    cleanup();
    useSpeech.setState({ status: { ...useSpeech.getState().status!, config: { model: 'parakeet-tdt-0.6b-v3', languages: ['tr', 'en'], vadThreshold: 0.55 } } });
    render(<AudioPage />);
    expect(screen.getByTestId('unsupported-langs')).toHaveTextContent('Le turc n’est pas reconnu');
    expect(screen.getByText('Actif')).toBeInTheDocument();
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

describe('translation card', () => {
  it('shows the state of the engine, its metrics and no other translation engine', async () => {
    const { useTranslation } = await import('../translation/translation');
    useTranslation.setState({ native: true, status: { config: { enabled: true, glossary: [], target: 'fr', sources: ['ru', 'en'] }, ready: true, problem: null, queued: 0 }, times: [310, 420] });
    render(<AudioPage />);
    expect(screen.getByTestId('translation-state')).toHaveTextContent('RU, EN → FR');
    expect(screen.getByTestId('translation-metrics')).toHaveTextContent('Traduction p50 420 ms');
    expect(screen.getByTestId('translation-card').textContent).not.toMatch(/opus|helsinki|DeepL|Azure/i);
    fireEvent.click(screen.getByRole('switch', { name: 'Traduire les voix et le chat' }));
    expect(useApp.getState().tg.translate).toBe(false);
  });

  it('says why nothing is translated without an engine', async () => {
    const { useTranslation } = await import('../translation/translation');
    useTranslation.setState({ native: true, status: { config: { enabled: true, glossary: [] }, ready: false, problem: 'aucun moteur de traduction connecté (Audio & IA › Traduction IA)', queued: 0 }, times: [] });
    render(<AudioPage />);
    expect(screen.getByTestId('translation-state')).toHaveTextContent('aucun moteur connecté');
    expect(screen.getByTestId('translation-problem')).toHaveTextContent('Audio & IA › Traduction IA');
  });

  it('offers to delete the old Opus-MT folders, only on request', async () => {
    const { useTranslation } = await import('../translation/translation');
    const ipc = await import('../../shared/ipc');
    useTranslation.setState({ native: true, status: { config: { enabled: true, glossary: [] }, ready: true, problem: null, queued: 0 }, times: [] });
    vi.spyOn(ipc.translation, 'legacyModels').mockResolvedValue({ folders: 2, bytes: 612_000_000 });
    const cleanup = vi.spyOn(ipc.translation, 'legacyCleanup').mockResolvedValue({ folders: 0, bytes: 0 });
    render(<AudioPage />);
    expect(await screen.findByTestId('legacy-models')).toHaveTextContent('2 dossiers, 612 Mo');
    expect(cleanup).not.toHaveBeenCalled();
    await act(async () => fireEvent.click(screen.getByText('Libérer l’espace')));
    expect(cleanup).toHaveBeenCalledTimes(1);
    await waitFor(() => expect(screen.queryByTestId('legacy-models')).toBeNull());
    vi.restoreAllMocks();
  });
});

describe('translation test', () => {
  it('says which engine translated the sentence', async () => {
    const { useTranslation } = await import('../translation/translation');
    const ipc = await import('../../shared/ipc');
    useTranslation.setState({ native: true, status: { config: { enabled: true, glossary: [] }, ready: true, problem: null, queued: 0 }, times: [] });
    const result = { id: 0, lang: 'ru', original: 'Двое идут на B', translated: 'Deux joueurs arrivent sur B.', error: null, ms: 640, queueMs: 0, cached: false, glossary: true, target: 'fr' };
    const spy = vi.spyOn(ipc.translation, 'test').mockResolvedValueOnce({ ...result, engine: 'llm' }).mockResolvedValueOnce({ ...result, ms: 2, engine: 'lexicon' });
    render(<AudioPage />);
    const translate = () => within(screen.getByTestId('translation-card')).getByText('Traduire');
    await act(async () => fireEvent.click(translate()));
    expect(await screen.findByTestId('translation-result')).toHaveTextContent('« Deux joueurs arrivent sur B. » · 640 ms · LLM');
    await act(async () => fireEvent.click(translate()));
    await waitFor(() => expect(screen.getByTestId('translation-result')).toHaveTextContent('lexique Counter-Strike'));
    expect(spy).toHaveBeenCalledTimes(2);
    spy.mockRestore();
  });
});

describe('Focus voix', () => {
  it('says what it does', () => {
    expect(focusNote(false, null, true)).toMatch(/Désactivé/);
    expect(focusNote(true, null, false)).toMatch(/dès que la capture démarre/);
    expect(focusNote(true, null, true)).toMatch(/source mono/);
    expect(focusNote(true, 6.25, true)).toBe('Activé · 6,3 dB de sons latéraux retirés sur la dernière seconde.');
  });
});
