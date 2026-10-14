import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { IDLE_CAPTURE, audioCapture, type CaptureStatus } from '../../shared/ipc';
import { captureDiag } from '../diagnostics/DiagPage';
import { captureActionFor, captureDegraded } from '../../app/native';
import {
  CAPTURE_TEST_MS,
  captureModeFor,
  captureSummary,
  cs2Summary,
  formatDb,
  levelToUnit,
  recordTestLevel,
  runCaptureTest,
  testMessage,
  useAudio,
} from './audio';

const initial = useAudio.getState();
const capturing = (over: Partial<CaptureStatus> = {}): CaptureStatus => ({
  ...IDLE_CAPTURE,
  state: 'capturing',
  mode: 'auto',
  source: { kind: 'process', pid: 4242, processName: 'cs2.exe' },
  format: { sampleRate: 48000, channels: 2, sampleFormat: 'f32' },
  ...over,
});

beforeEach(() => useAudio.setState(initial, true));
afterEach(() => {
  vi.useRealTimers();
  vi.restoreAllMocks();
});

describe('capture helpers', () => {
  it('maps UI choices to Rust capture modes', () => {
    expect(captureModeFor('process')).toBe('auto');
    expect(captureModeFor('device')).toBe('device');
    expect(captureModeFor('wav')).toBe('auto');
  });

  it('maps dBFS to a 0..1 meter', () => {
    expect(levelToUnit(-90)).toBe(0);
    expect(levelToUnit(-30)).toBe(0.5);
    expect(levelToUnit(0)).toBe(1);
    expect(levelToUnit(6)).toBe(1);
  });

  it('formats dB the French way', () => {
    expect(formatDb(-12.34)).toBe('−12,3 dBFS');
    expect(formatDb(null)).toBe('−∞ dBFS');
    expect(formatDb(-90)).toBe('−∞ dBFS');
  });

  it('summarises a process capture', () => {
    const s = captureSummary(capturing());
    expect(s).toMatchObject({ value: 'Process loopback', short: 'cs2.exe', color: '#7fe0a8' });
    expect(s.detail).toContain('PID 4242');
    expect(s.detail).toContain('48 kHz');
  });

  it('flags the device fallback', () => {
    const s = captureSummary(capturing({ source: { kind: 'device', id: 'x', name: 'Casque' }, fallbackReason: 'E_NOTIMPL' }));
    expect(s.value).toBe('Loopback système');
    expect(s.detail).toMatch(/^Repli/);
    expect(s.color).toBe('#ffa24a');
  });

  it('explains errors and stops', () => {
    expect(captureSummary({ ...IDLE_CAPTURE, state: 'error', error: { code: 'cs2_not_running', message: 'CS2 absent' } }).detail).toBe('CS2 absent');
    expect(captureSummary({ ...IDLE_CAPTURE, stopReason: 'emergency' }).detail).toMatch(/urgence/);
    expect(captureSummary({ ...IDLE_CAPTURE, stopReason: 'cs2Exited' }).detail).toMatch(/CS2 fermé/);
  });

  it('summarises CS2 detection', () => {
    expect(cs2Summary(null).value).toBe('…');
    expect(cs2Summary({ running: true, pid: 12 })).toMatchObject({ value: 'Détecté', detail: 'cs2.exe · PID 12' });
    expect(cs2Summary({ running: false, pid: null }).value).toBe('Non détecté');
  });
});

describe('session ↔ capture sync', () => {
  it('starts with the session and stops on pause, stop or emergency', () => {
    expect(captureActionFor('IDLE', 'INITIALIZING')).toBe('start');
    expect(captureActionFor('INITIALIZING', 'LISTENING')).toBeNull();
    expect(captureActionFor('LISTENING', 'DEGRADED')).toBeNull();
    expect(captureActionFor('LISTENING', 'PAUSED')).toBe('stop');
    expect(captureActionFor('PAUSED', 'LISTENING')).toBe('start');
    expect(captureActionFor('LISTENING', 'STOPPED')).toBe('stop');
    expect(captureActionFor('STOPPED', 'STOPPED')).toBeNull();
  });

  it('degrades the session on fallback or error only', () => {
    expect(captureDegraded(capturing())).toBe(false);
    expect(captureDegraded(capturing({ fallbackReason: 'x' }))).toBe(true);
    expect(captureDegraded({ ...IDLE_CAPTURE, state: 'error' })).toBe(true);
    expect(captureDegraded(IDLE_CAPTURE)).toBe(false);
  });
});

describe('capture test', () => {
  it('opens the source for 10 s, records the real peak, then closes it', async () => {
    vi.useFakeTimers();
    const start = vi.spyOn(audioCapture, 'start').mockResolvedValue(capturing());
    const stop = vi.spyOn(audioCapture, 'stop').mockResolvedValue(IDLE_CAPTURE);
    await runCaptureTest('process');
    expect(start).toHaveBeenCalledWith('auto', null);
    expect(useAudio.getState().test.running).toBe(true);

    recordTestLevel({ rms: 0.1, peak: 0.2, rmsDb: -20, peakDb: -14 });
    recordTestLevel({ rms: 0.01, peak: 0.02, rmsDb: -40, peakDb: -34 });
    await vi.advanceTimersByTimeAsync(CAPTURE_TEST_MS + 300);

    const t = useAudio.getState().test;
    expect(t).toMatchObject({ running: false, done: true, progress: 100, maxPeakDb: -14 });
    expect(testMessage(t)).toBe('Signal reçu · crête −14,0 dBFS');
    expect(stop).toHaveBeenCalledOnce();
  });

  it('reuses a running capture without stopping it', async () => {
    vi.useFakeTimers();
    useAudio.setState({ status: capturing() });
    const start = vi.spyOn(audioCapture, 'start');
    const stop = vi.spyOn(audioCapture, 'stop');
    await runCaptureTest('device');
    await vi.advanceTimersByTimeAsync(CAPTURE_TEST_MS + 300);
    expect(start).not.toHaveBeenCalled();
    expect(stop).not.toHaveBeenCalled();
    expect(testMessage(useAudio.getState().test)).toMatch(/Aucun signal/);
  });

  it('reports a capture that cannot open', async () => {
    vi.spyOn(audioCapture, 'start').mockResolvedValue({
      ...IDLE_CAPTURE,
      state: 'error',
      error: { code: 'cs2_not_running', message: 'Counter-Strike 2 n’est pas lancé' },
    });
    await runCaptureTest('process');
    const t = useAudio.getState().test;
    expect(t).toMatchObject({ running: false, done: true });
    expect(testMessage(t)).toBe('Counter-Strike 2 n’est pas lancé');
  });

  it('ignores levels outside a test', () => {
    recordTestLevel({ rms: 1, peak: 1, rmsDb: 0, peakDb: 0 });
    expect(useAudio.getState().test.maxPeakDb).toBeNull();
  });
});

describe('diagnostic row', () => {
  const platform = { windowsBuild: 19045, processLoopbackDocumented: false };

  it('shows the real build and the captured process', () => {
    const [detail, st] = captureDiag({ platform: { windowsBuild: 26100, processLoopbackDocumented: true }, status: capturing() });
    expect(detail).toBe('Build 26100 · cs2.exe ciblé (PID 4242)');
    expect(st).toBe('OK');
  });

  it('warns on fallback and untested old builds, fails on error', () => {
    expect(captureDiag({ platform, status: capturing({ source: { kind: 'device', id: 'd', name: 'C' }, fallbackReason: 'E_NOTIMPL' }) })[1]).toBe('AVERT.');
    expect(captureDiag({ platform, status: IDLE_CAPTURE })).toEqual(['Build 19045 < 20348 (non documenté, tenté) · non testé dans cette session', 'AVERT.']);
    expect(captureDiag({ platform, status: { ...IDLE_CAPTURE, state: 'error', error: { code: 'x', message: 'boom' } } })[1]).toBe('ÉCHEC');
  });
});
