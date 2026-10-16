import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { voice, VOICE_ENGINE_BYTES, type DictationState, type VoiceStatus } from '../../shared/ipc';
import { useApp, VOICE_DEFAULTS } from '../../app/state/store';
import { restore, snapshot } from '../../app/state/persist';
import { dictationView } from '../reply/DictationPanel';
import { VoiceCard } from './VoiceCard';
import { chosenVoice, downloadPercent, pttView, toDownload, useVoice, validPttKey, voiceSummary, voicesOf } from './voice';

// Simulated Rust states: no audio device, no Piper process in these tests.
const VOICES = [
  { id: 'ru_RU-irina-medium', lang: 'ru', name: 'Irina', gender: 'female' as const, onnxBytes: 63_201_294, jsonBytes: 4_765, installed: false },
  { id: 'ru_RU-dmitri-medium', lang: 'ru', name: 'Dmitri', gender: 'male' as const, onnxBytes: 63_201_294, jsonBytes: 4_824, installed: true },
  { id: 'en_US-amy-medium', lang: 'en', name: 'Amy', gender: 'female' as const, onnxBytes: 63_201_294, jsonBytes: 4_882, installed: false },
];
const status = (p: Partial<VoiceStatus> = {}): VoiceStatus => ({
  options: VOICE_DEFAULTS,
  engineReady: false,
  voices: VOICES,
  cable: null,
  mics: [{ id: 'mic-1', name: 'Micro (HyperX Cloud II)', isDefault: true }],
  routing: false,
  speaking: false,
  error: null,
  download: { state: 'idle', current: null, doneBytes: 0, totalBytes: 0, missingFiles: 0, missingBytes: 0, error: null },
  ptt: { phase: 'idle', detail: null, pressed: false },
  ...p,
});

const app0 = useApp.getState();
const voice0 = useVoice.getState();
beforeEach(() => {
  useApp.setState(app0, true);
  useVoice.setState(voice0, true);
});
afterEach(() => vi.restoreAllMocks());

describe('AI voice helpers', () => {
  it('picks the chosen voice of each language and sums what is left to download', () => {
    const st = status();
    expect(voicesOf(VOICES, 'ru').map((v) => v.name)).toEqual(['Irina', 'Dmitri']);
    expect(chosenVoice(VOICES, VOICE_DEFAULTS, 'ru')?.name).toBe('Irina');
    const opts = { ...VOICE_DEFAULTS, voices: { ru: 'ru_RU-dmitri-medium' } };
    expect(chosenVoice(VOICES, opts, 'ru')?.name).toBe('Dmitri');
    // Dmitri is installed: only Amy and the engine are missing.
    expect(toDownload(st, opts, ['ru', 'en'])).toEqual({ ids: ['en_US-amy-medium'], bytes: 63_201_294 + 4_882 + VOICE_ENGINE_BYTES });
    expect(toDownload(status({ engineReady: true }), opts, ['ru'])).toEqual({ ids: [], bytes: 0 });
    expect(downloadPercent({ ...st.download, state: 'downloading', doneBytes: 50, totalBytes: 200 })).toBe(25);
    expect(downloadPercent(st.download)).toBeNull();
  });

  it('says what is missing before the voice can speak', () => {
    const on = { ...VOICE_DEFAULTS, enabled: true };
    expect(voiceSummary(status(), VOICE_DEFAULTS, true).value).toBe('Désactivée');
    expect(voiceSummary(status(), on, true).value).toBe('VB-CABLE introuvable');
    const cable = { id: 'c', name: 'CABLE Input (VB-Audio Virtual Cable)', isDefault: false };
    expect(voiceSummary(status({ cable }), on, false).value).toBe('Voix à télécharger');
    expect(voiceSummary(status({ cable, error: 'micro : introuvable' }), on, true)).toMatchObject({ value: 'Problème', detail: 'micro : introuvable' });
    expect(voiceSummary(status({ cable, routing: true }), on, true).value).toBe('Prête');
    expect(voiceSummary(status({ cable, routing: true, speaking: true }), on, true).value).toBe('La voix parle…');
  });

  it('is off by default, saved with the settings and checked on restore', () => {
    expect(app0.voice.enabled).toBe(false);
    const v = { enabled: true, mic: 'mic-1', voices: { ru: 'ru_RU-dmitri-medium' }, monitor: false, volume: 120, micVolume: 80, autoPtt: true, pttKey: 'V', pttLeadMs: 300, pttTailMs: 500, speakTyped: true };
    const snap = snapshot({ ...app0, voice: v }, null);
    expect(restore(JSON.parse(JSON.stringify(snap.config)), app0).app.voice).toEqual(v);
    const bad = restore({ version: 1, settings: { voice: { enabled: 'yes', mic: 4, voices: { russian: 'x', ru: 7 }, volume: 900 } } }, app0).app.voice;
    expect(bad).toEqual(VOICE_DEFAULTS);
    // Automatic push-to-talk: off and K by default; settings of earlier versions get them.
    expect(VOICE_DEFAULTS).toMatchObject({ autoPtt: false, pttKey: 'K', speakTyped: false });
    const old = restore({ version: 1, settings: { voice: { enabled: true, volume: 90 } } }, app0).app.voice;
    expect(old).toMatchObject({ enabled: true, autoPtt: false, pttKey: 'K', pttLeadMs: 250, pttTailMs: 350, speakTyped: false });
    const evil = restore({ version: 1, settings: { voice: { autoPtt: 'yes', pttKey: 'K; rm', pttLeadMs: 5, pttTailMs: 99999 } } }, app0).app.voice;
    expect(evil).toMatchObject({ autoPtt: false, pttKey: 'K', pttLeadMs: 250, pttTailMs: 350 });
  });

  it('accepts the same key names as the Rust side and describes each step', () => {
    for (const k of ['K', 'v', '4', 'F5', 'f12', 'CapsLock', 'LAlt', 'Mouse4', 'Space']) expect(validPttKey(k)).toBe(true);
    for (const k of ['', 'KK', 'F13', 'Échap', 'Mouse9']) expect(validPttKey(k)).toBe(false);
    expect(pttView(null).step).toBe(-1);
    expect(pttView({ phase: 'preparing', detail: null, pressed: false }).step).toBe(0);
    expect(pttView({ phase: 'held', detail: null, pressed: true }).step).toBe(1);
    expect(pttView({ phase: 'playing', detail: null, pressed: true })).toMatchObject({ step: 2, busy: true });
    expect(pttView({ phase: 'done', detail: null, pressed: true })).toMatchObject({ step: 3, text: 'Phrase transmise, touche relâchée.', busy: false });
    expect(pttView({ phase: 'done', detail: 'CS2 n’est pas au premier plan : touche non appuyée', pressed: false }).text).toContain('premier plan');
    expect(pttView({ phase: 'error', detail: 'CS2 n’est plus au premier plan', pressed: true })).toMatchObject({ step: -1, color: '#ff8a7a' });
    expect(pttView({ phase: 'cancelled', detail: null, pressed: true }).text).toBe('Transmission annulée, touche relâchée.');
  });

  it('tells when a dictated reply was said in the game', () => {
    const st: DictationState = { state: 'done', level: 0, seconds: 0, text: 'Deux en B', translated: 'Двое на B', target: 'ru', via: 'phrasebook', copied: true, error: null, spoken: true };
    expect(dictationView(st).detail).toContain('Dit en jeu par la voix IA');
    expect(dictationView({ ...st, spoken: false }).detail).toContain('Ctrl+V');
  });
});

describe('AI voice card', () => {
  it('guides through the cable, the microphone and the voices', async () => {
    useVoice.setState({ native: true, status: status() });
    vi.spyOn(voice, 'status').mockResolvedValue(status());
    const open = vi.spyOn(voice, 'openCablePage').mockResolvedValue();
    const download = vi.spyOn(voice, 'download').mockResolvedValue(status().download);
    render(<VoiceCard />);
    expect(screen.getByTestId('voice-state')).toHaveTextContent('Désactivée');
    fireEvent.click(screen.getByText('Télécharger VB-CABLE ↗'));
    expect(open).toHaveBeenCalled();
    fireEvent.change(screen.getByLabelText('Micro'), { target: { value: 'mic-1' } });
    expect(useApp.getState().voice.mic).toBe('mic-1');
    // Browser preview: the reply languages are English and Russian.
    expect(await screen.findByTestId('voice-lang-ru')).toHaveTextContent('Русский');
    expect(screen.getByTestId('voice-download')).toHaveTextContent('Moteur Piper + 2 voix');
    await act(async () => fireEvent.click(screen.getByText('Télécharger')));
    expect(download).toHaveBeenCalledWith(['en_US-amy-medium', 'ru_RU-irina-medium']);
    fireEvent.change(screen.getByLabelText('Voix Русский'), { target: { value: 'ru_RU-dmitri-medium' } });
    expect(useApp.getState().voice.voices).toEqual({ ru: 'ru_RU-dmitri-medium' });
    fireEvent.click(screen.getByRole('switch', { name: 'Dire mes réponses en jeu avec une voix IA' }));
    expect(useApp.getState().voice.enabled).toBe(true);
  });

  it('previews an installed voice and says the test sentence in the game once ready', async () => {
    const cable = { id: 'c', name: 'CABLE Input (VB-Audio Virtual Cable)', isDefault: false };
    const ready = status({ cable, routing: true, engineReady: true, voices: VOICES.map((v) => ({ ...v, installed: true })) });
    useVoice.setState({ native: true, status: ready });
    useApp.setState({ voice: { ...VOICE_DEFAULTS, enabled: true } });
    vi.spyOn(voice, 'status').mockResolvedValue(ready);
    const preview = vi.spyOn(voice, 'preview').mockResolvedValue(1800);
    const say = vi.spyOn(voice, 'say').mockResolvedValue(2100);
    render(<VoiceCard />);
    expect(screen.getByTestId('voice-step-1')).toHaveTextContent('CABLE Input (VB-Audio Virtual Cable)');
    expect(screen.getByTestId('voice-state')).toHaveTextContent('Prête');
    expect(screen.queryByTestId('voice-download')).toBeNull();
    const ru = await screen.findByTestId('voice-lang-ru');
    await act(async () => fireEvent.click(ru.querySelector('[role=button]')!));
    expect(preview).toHaveBeenCalledWith('ru_RU-irina-medium');
    await act(async () => fireEvent.click(screen.getAllByText('Dire en jeu')[1]));
    expect(say).toHaveBeenCalledWith('Двое на B, осторожно, флешка.', 'ru');
    await waitFor(() => expect(screen.getByTestId('voice-state')).toHaveTextContent('Dite en jeu (2,1 s)'));
  });

  it('turns on automatic push-to-talk, checks the key, shows the steps, tests and cancels', async () => {
    const cable = { id: 'c', name: 'CABLE Input (VB-Audio Virtual Cable)', isDefault: false };
    const ready = status({ cable, routing: true, engineReady: true, voices: VOICES.map((v) => ({ ...v, installed: true })) });
    useVoice.setState({ native: true, status: ready });
    useApp.setState({ voice: { ...VOICE_DEFAULTS, enabled: true } });
    vi.spyOn(voice, 'status').mockResolvedValue(ready);
    const test = vi.spyOn(voice, 'pttTest').mockResolvedValue(2100);
    const cancel = vi.spyOn(voice, 'cancel').mockResolvedValue();
    render(<VoiceCard />);
    expect(screen.queryByLabelText('Touche vocale')).toBeNull();
    fireEvent.click(screen.getByRole('switch', { name: 'Appuyer automatiquement pour parler dans CS2' }));
    expect(useApp.getState().voice.autoPtt).toBe(true);
    const key = screen.getByLabelText('Touche vocale');
    expect(key).toHaveValue('K');
    expect(screen.getByTestId('voice-ptt')).toHaveTextContent('CS2 au premier plan');
    fireEvent.change(key, { target: { value: 'Échap' } });
    expect(screen.getByRole('alert')).toHaveTextContent('Touche non reconnue');
    fireEvent.change(key, { target: { value: 'V' } });
    expect(useApp.getState().voice.pttKey).toBe('V');
    expect(screen.queryByRole('alert')).toBeNull();
    act(() => useVoice.setState({ ptt: { phase: 'held', detail: null, pressed: true } }));
    expect(screen.getByText('Activation micro')).toHaveAttribute('data-active', 'true');
    act(() => useVoice.setState({ ptt: { phase: 'playing', detail: null, pressed: true } }));
    expect(screen.getByText('Transmission')).toHaveAttribute('data-active', 'true');
    await act(async () => fireEvent.click(screen.getByText('Tester dans 5 s')));
    expect(test).toHaveBeenCalledWith('en', 5);
    fireEvent.click(screen.getByText('Annuler la transmission'));
    expect(cancel).toHaveBeenCalled();
    act(() => useVoice.setState({ ptt: { phase: 'error', detail: 'CS2 n’est plus au premier plan : touche relâchée, phrase interrompue.', pressed: true } }));
    expect(screen.getByTestId('voice-ptt-state')).toHaveTextContent('Erreur : CS2 n’est plus au premier plan');
  });

  it('shows why the voice could not speak', async () => {
    const cable = { id: 'c', name: 'CABLE Input', isDefault: false };
    const ready = status({ cable, routing: true, engineReady: true, voices: VOICES.map((v) => ({ ...v, installed: true })) });
    useVoice.setState({ native: true, status: ready });
    vi.spyOn(voice, 'status').mockResolvedValue(ready);
    vi.spyOn(voice, 'say').mockRejectedValue('le moteur de voix ne répond pas');
    render(<VoiceCard />);
    await screen.findByTestId('voice-lang-en');
    await act(async () => fireEvent.click(screen.getAllByText('Dire en jeu')[0]));
    expect(await screen.findByRole('alert')).toHaveTextContent('le moteur de voix ne répond pas');
  });
});
