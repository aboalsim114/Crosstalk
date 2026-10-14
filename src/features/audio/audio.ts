// Real audio capture and CS2 state, mirrored from the Rust core (events
// `audio://status`, `audio://level`, `cs2://status`). Nothing here is simulated:
// outside the desktop app the store simply stays empty (`native: false`).

import { t, useLocale } from '../../shared/i18n';
import { create } from 'zustand';
import {
  IDLE_CAPTURE,
  audioCapture,
  isTauri,
  type AudioDevice,
  type CaptureMode,
  type CaptureStatus,
  type Cs2Status,
  type LevelReading,
  type PlatformInfo,
} from '../../shared/ipc';
import type { CaptureSource } from '../../app/state/store';

/** dB value below which the signal is considered absent. */
export const SIGNAL_THRESHOLD_DB = -50;
/** Duration of the capture test on the Audio & IA screen. */
export const CAPTURE_TEST_MS = 10_000;

export interface CaptureTest {
  running: boolean;
  /** 0–100. */
  progress: number;
  /** Highest peak seen during the test, in dBFS. */
  maxPeakDb: number | null;
  done: boolean;
  error: string | null;
}

export interface AudioState {
  native: boolean;
  platform: PlatformInfo | null;
  cs2: Cs2Status | null;
  status: CaptureStatus;
  level: LevelReading | null;
  devices: AudioDevice[];
  devicesError: string | null;
  /** Output device for Device mode (and Auto fallback); null = Windows default. */
  deviceId: string | null;
  test: CaptureTest;
}

const IDLE_TEST: CaptureTest = { running: false, progress: 0, maxPeakDb: null, done: false, error: null };

export const useAudio = create<AudioState>()(() => ({
  native: isTauri,
  platform: null,
  cs2: null,
  status: IDLE_CAPTURE,
  level: null,
  devices: [],
  devicesError: null,
  deviceId: null,
  test: IDLE_TEST,
}));

// ---------- pure helpers (unit-tested) ----------

/** Maps a UI capture choice to the Rust capture mode. */
export function captureModeFor(source: CaptureSource): CaptureMode {
  return source === 'device' ? 'device' : 'auto';
}

/** Maps a dBFS value to 0..1 for meters and the 3D wave (−60 dB → 0, 0 dB → 1). */
export function levelToUnit(db: number): number {
  return Math.min(1, Math.max(0, (db + 60) / 60));
}

export function isCapturing(s: CaptureStatus): boolean {
  return s.state === 'capturing';
}

export interface CaptureSummary {
  /** Short value, e.g. "Process loopback". */
  value: string;
  /** Detail line. */
  detail: string;
  /** Dot / accent colour. */
  color: string;
  /** Pipeline sub-label, e.g. "cs2.exe" or "loopback". */
  short: string;
}

export function captureSummary(s: CaptureStatus): CaptureSummary {
  switch (s.state) {
    case 'capturing': {
      const fmt = s.format ? ` · ${(s.format.sampleRate / 1000).toLocaleString(useLocale.getState().locale)} kHz` : '';
      if (s.source?.kind === 'process')
        return { value: t('cap.process'), detail: t('cap.processDetail', { name: s.source.processName, pid: s.source.pid, fmt }), color: '#7fe0a8', short: s.source.processName };
      const name = s.source?.kind === 'device' ? s.source.name : t('cap.device');
      return s.fallbackReason
        ? { value: t('cap.system'), detail: t('cap.fallbackDetail', { name, fmt }), color: '#ffa24a', short: 'loopback' }
        : { value: t('cap.system'), detail: t('cap.deviceDetail', { name, fmt }), color: '#ffa24a', short: 'loopback' };
    }
    case 'starting':
      return { value: t('cap.opening'), detail: t('cap.openingDetail'), color: '#7fd4ff', short: '…' };
    case 'error':
      return { value: t('cap.unavailable'), detail: s.error?.message ?? t('cap.unknownError'), color: '#ff8a7a', short: t('cap.error') };
    default: {
      const why = s.stopReason === 'emergency' ? t('cap.emergency') : s.stopReason === 'cs2Exited' ? t('cap.cs2Exited') : t('cap.withSession');
      return { value: t('cap.inactive'), detail: why, color: '#8f8a83', short: t('cap.inactiveShort') };
    }
  }
}

export function cs2Summary(c: Cs2Status | null): { value: string; detail: string; color: string } {
  if (!c) return { value: '…', detail: t('cs2.searching'), color: '#5a5650' };
  return c.running && c.pid !== null
    ? { value: t('cs2.detected'), detail: `cs2.exe · PID ${c.pid}`, color: '#7fe0a8' }
    : { value: t('cs2.notDetected'), detail: t('cs2.launch'), color: '#8f8a83' };
}

export function testMessage(t: CaptureTest): string {
  if (t.error) return t.error;
  if (t.running) return `${Math.round((t.progress / 100) * (CAPTURE_TEST_MS / 1000))} / ${CAPTURE_TEST_MS / 1000} s · crête ${formatDb(t.maxPeakDb)}`;
  if (t.done)
    return t.maxPeakDb !== null && t.maxPeakDb > SIGNAL_THRESHOLD_DB
      ? `Signal reçu · crête ${formatDb(t.maxPeakDb)}`
      : 'Aucun signal : le jeu émet-il du son ? Vérifiez la source choisie.';
  return 'Ouvre la source choisie pendant 10 s et mesure le niveau réel';
}

export function formatDb(db: number | null): string {
  if (db === null || db <= -90) return '−∞ dBFS';
  return `${db.toFixed(1).replace('.', ',').replace('-', '−')} dBFS`;
}

// ---------- actions ----------

export async function refreshDevices(): Promise<void> {
  try {
    const devices = await audioCapture.devices();
    useAudio.setState({ devices, devicesError: null });
  } catch (e) {
    useAudio.setState({ devices: [], devicesError: String(e) });
  }
}

let testTimer: ReturnType<typeof setInterval> | undefined;

/**
 * Opens the selected source for 10 s (unless a capture already runs) and
 * records the highest real peak. `onLevel` in the bridge feeds `maxPeakDb`.
 */
export async function runCaptureTest(source: CaptureSource): Promise<void> {
  const st = useAudio.getState();
  if (st.test.running) return;
  const ownCapture = !isCapturing(st.status);
  useAudio.setState({ test: { ...IDLE_TEST, running: true } });
  if (ownCapture) {
    const res = await audioCapture.start(captureModeFor(source), st.deviceId);
    if (res.state !== 'capturing') {
      useAudio.setState({ test: { ...IDLE_TEST, done: true, error: res.error?.message ?? 'Capture impossible' } });
      return;
    }
  }
  const t0 = Date.now();
  clearInterval(testTimer);
  testTimer = setInterval(async () => {
    const progress = Math.min(100, ((Date.now() - t0) / CAPTURE_TEST_MS) * 100);
    const test = useAudio.getState().test;
    if (progress < 100) {
      useAudio.setState({ test: { ...test, progress } });
      return;
    }
    clearInterval(testTimer);
    useAudio.setState({ test: { ...test, progress: 100, running: false, done: true } });
    if (ownCapture) await audioCapture.stop();
  }, 250);
}

/** Folds a level reading into the running test. */
export function recordTestLevel(l: LevelReading): void {
  const test = useAudio.getState().test;
  if (!test.running) return;
  useAudio.setState({ test: { ...test, maxPeakDb: Math.max(test.maxPeakDb ?? -90, l.peakDb) } });
}
