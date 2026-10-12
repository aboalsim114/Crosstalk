// Wires the store to the native side: global shortcuts / tray actions in, and the
// in-game overlay window out. Only active inside the Tauri app.

import { audioCapture, cs2, isTauri, onNativeAction, overlayWindow, speech, translation, type CaptureStatus, type NativeAction, type OverlayWindowConfig } from './backend';
import { refreshModels, speechConfigFrom, transcriptEntry, useSpeech } from './state/speech';
import { recordTime, translationConfigFrom, useTranslation } from './state/translation';
import { captureModeFor, levelToUnit, recordTestLevel, refreshDevices, useAudio } from './state/audio';
import { isRunning, useApp, type AppState, type FeedEntry, type Session } from './state/store';

export interface OverlayPayload {
  feed: FeedEntry[];
  maxBlocks: number;
  fmt: AppState['fmt'];
  opacity: number;
  textSize: number;
  /** Seconds before a line fades out. */
  duration: number;
  corner: AppState['corner'];
  /** Repositioning: show placeholder lines and a dashed frame, accept drags. */
  repos: boolean;
}

export function overlayPayload(s: AppState): OverlayPayload {
  return {
    feed: s.feed.slice(0, 3),
    maxBlocks: s.maxBlocks,
    fmt: s.fmt,
    opacity: s.opacity,
    textSize: s.textSize,
    duration: s.duration,
    corner: s.corner,
    repos: s.repos,
  };
}

/** Window flags derived from the settings: shown while a session runs (or when repositioning). */
export function overlayConfig(s: AppState, prev?: AppState): OverlayWindowConfig {
  return {
    visible: s.overlayOn && (isRunning(s) || s.repos),
    clickThrough: s.tg.click && !s.repos,
    corner: !prev || prev.corner !== s.corner ? s.corner : null,
  };
}

export function applyNativeAction(a: NativeAction) {
  const s = useApp.getState();
  ({ overlay: s.toggleOverlay, pause: s.togglePause, emergency: s.emergency, session: s.toggleSession })[a]();
}

/** Sessions during which the real capture runs. */
export const capturesDuring = (s: Session) => s === 'INITIALIZING' || s === 'LISTENING' || s === 'DEGRADED';

/** What the capture must do when the session goes from `prev` to `next`. */
export function captureActionFor(prev: Session, next: Session): 'start' | 'stop' | null {
  const was = capturesDuring(prev);
  const is = capturesDuring(next);
  return is && !was ? 'start' : was && !is ? 'stop' : null;
}

/** A capture that runs on the fallback device, or failed, degrades the session. */
export const captureDegraded = (c: CaptureStatus) => c.state === 'error' || (c.state === 'capturing' && c.fallbackReason !== null);

/** Mirrors the Rust capture into the stores and drives it from the session. */
function startAudioBridge(): Promise<() => void>[] {
  const unlisten: Promise<() => void>[] = [];
  audioCapture.status().then((status) => useAudio.setState({ status }));
  audioCapture.platform().then((platform) => useAudio.setState({ platform }));
  cs2.status().then((c) => useAudio.setState({ cs2: c }));
  refreshDevices();

  unlisten.push(
    audioCapture.onStatus((status) => {
      useAudio.setState({ status, ...(status.state === 'capturing' ? {} : { level: null }) });
      if (status.state !== 'capturing') useApp.setState({ level: 0 });
      const app = useApp.getState();
      if (app.session === 'LISTENING' && captureDegraded(status)) useApp.setState({ session: 'DEGRADED' });
    }),
  );
  unlisten.push(
    audioCapture.onLevel((level) => {
      useAudio.setState({ level });
      recordTestLevel(level);
      useApp.setState({ level: levelToUnit(level.rmsDb) });
    }),
  );
  unlisten.push(cs2.onStatus((c) => useAudio.setState({ cs2: c })));

  const unsub = useApp.subscribe((next, prev) => {
    if (next.session === 'LISTENING' && captureDegraded(useAudio.getState().status)) {
      useApp.setState({ session: 'DEGRADED' });
    }
    const action = captureActionFor(prev.session, next.session);
    if (action === 'stop') audioCapture.stop();
    if (action === 'start') {
      audioCapture.start(captureModeFor(next.capture), useAudio.getState().deviceId).then((res) => {
        if (!capturesDuring(useApp.getState().session)) {
          audioCapture.stop();
          return;
        }
        if (res.state === 'error') useApp.getState().flash(`Capture audio indisponible : ${res.error?.message ?? 'erreur'}`);
        else if (res.fallbackReason) useApp.getState().flash('Capture par processus indisponible · repli sur le périphérique de sortie');
      });
    }
  });
  unlisten.push(Promise.resolve(unsub));
  return unlisten;
}

/** Settings that the Rust speech service needs, as a comparable key. */
export const speechConfigKey = (s: AppState) => JSON.stringify(speechConfigFrom(s));

/** Mirrors the Rust speech recognition: status, real transcripts, voice activity. */
/** Settings that the Rust translation service needs, as a comparable key. */
export const translationConfigKey = (s: AppState) => JSON.stringify(translationConfigFrom(s));

/** Mirrors the local translation: model states, and each French line onto its transcript. */
function startTranslationBridge(): Promise<() => void>[] {
  const unlisten: Promise<() => void>[] = [];
  translation.status().then((status) => status && useTranslation.setState({ status }));
  unlisten.push(translation.onStatus((status) => useTranslation.setState({ status })));
  unlisten.push(
    translation.onResult((t) => {
      recordTime(t);
      useApp.getState().applyTranslation(t);
    }),
  );
  let lastKey = '';
  let timer: ReturnType<typeof setTimeout> | undefined;
  const push = (s: AppState) => {
    const key = translationConfigKey(s);
    if (key === lastKey) return;
    lastKey = key;
    clearTimeout(timer);
    timer = setTimeout(() => translation.configure(translationConfigFrom(s)).then((status) => status && useTranslation.setState({ status })), 250);
  };
  push(useApp.getState());
  unlisten.push(Promise.resolve(useApp.subscribe((s) => push(s))));
  return unlisten;
}

function startSpeechBridge(): Promise<() => void>[] {
  const unlisten: Promise<() => void>[] = [];
  speech.status().then((status) => useSpeech.setState({ status }));
  speech.stats().then((stats) => useSpeech.setState({ stats }));
  refreshModels();
  unlisten.push(
    speech.onStatus((status) => {
      useSpeech.setState({ status });
      refreshModels();
    }),
  );
  unlisten.push(speech.onStats((stats) => useSpeech.setState({ stats })));
  unlisten.push(speech.onActivity((activity) => useSpeech.setState({ activity })));
  unlisten.push(
    speech.onTranscript((t) => {
      const app = useApp.getState();
      if (capturesDuring(app.session)) app.addLine(transcriptEntry(t));
    }),
  );

  // Model, VAD threshold, languages and glossary follow the UI settings.
  let lastKey = '';
  let timer: ReturnType<typeof setTimeout> | undefined;
  const push = (s: AppState) => {
    const key = speechConfigKey(s);
    if (key === lastKey) return;
    lastKey = key;
    clearTimeout(timer);
    timer = setTimeout(() => speech.configure(speechConfigFrom(s)).then((status) => status && useSpeech.setState({ status })), 250);
  };
  push(useApp.getState());
  unlisten.push(Promise.resolve(useApp.subscribe((s) => push(s))));
  return unlisten;
}

export function startNativeBridge(): () => void {
  if (!isTauri) return () => {};
  const unlisten: Promise<() => void>[] = [];
  unlisten.push(onNativeAction(applyNativeAction));
  unlisten.push(...startAudioBridge());
  unlisten.push(...startSpeechBridge());
  unlisten.push(...startTranslationBridge());

  const push = () => overlayWindow.pushState(overlayPayload(useApp.getState()));
  unlisten.push(overlayWindow.onReady(push));

  let lastCfg = '';
  const sync = (s: AppState, prev?: AppState) => {
    const cfg = overlayConfig(s, prev);
    const key = JSON.stringify(cfg);
    if (key !== lastCfg) {
      lastCfg = key;
      overlayWindow.configure(cfg).catch((e) => console.warn('overlay_configure failed', e));
    }
    if (!prev || prev.feed !== s.feed || prev.fmt !== s.fmt || prev.opacity !== s.opacity || prev.textSize !== s.textSize || prev.maxBlocks !== s.maxBlocks || prev.duration !== s.duration || prev.corner !== s.corner || prev.repos !== s.repos) push();
  };
  sync(useApp.getState());
  const unsub = useApp.subscribe(sync);

  return () => {
    unsub();
    unlisten.forEach((p) => p.then((f) => f()));
  };
}
