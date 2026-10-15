// Wires the store to the native side: global shortcuts / tray actions in, and the
// in-game overlay window out. Only active inside the Tauri app.

import { resolveLocale, type Locale } from '../shared/i18n';
import { audioCapture, chat, cs2, dictation, isTauri, llm, modelDownload, onNativeAction, overlayWindow, reply, speech, translation, type CaptureStatus, type NativeAction, type OverlayWindowConfig } from '../shared/ipc';
import { refreshModels, speechConfigFrom, transcriptEntry, useSpeech } from '../features/speech/speech';
import { recordTime, translationConfigFrom, useTranslation } from '../features/translation/translation';
import { llmConfigFrom, refreshLlm, useLlm } from '../features/translation/llm';
import { setupNeeds, startSetup, useSetup } from '../features/setup/setup';
import { chatConfigFrom, chatEntry, useChat } from '../features/chat/chat';
import { captureModeFor, levelToUnit, recordTestLevel, refreshDevices, useAudio } from '../features/audio/audio';
import { recentLangs } from '../features/languages/langs';
import { radarPayload, type RadarPayload } from '../features/radar/radar';
import { useGsi } from '../features/recap/gsi';
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
  /** Interface language of the labels ("VOIX • RUSSE", quotes…). */
  locale: Locale;
  /** Language of the translations ("RUSSE → FRANÇAIS"). */
  target: string;
  /** Callout radar: map and lit zones, null when off or on an unknown map. */
  radar: RadarPayload | null;
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
    locale: resolveLocale(s.uiLang),
    target: s.target,
    radar: radarPayload(s.radar, useGsi.getState().status?.map, s.feed, s.radarDemo, Date.now()),
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

/**
 * Sends the overlay window's flags to the Rust core when they change. The
 * first call can come before the core has created the overlay window (the
 * main window loads while the core is still starting): a failed call is
 * retried at the next change, and the overlay's "ready" resends everything,
 * including the corner saved in the settings.
 */
export function overlayFlagsSync(configure: (cfg: OverlayWindowConfig) => Promise<unknown>) {
  let lastCfg = '';
  const send = (cfg: OverlayWindowConfig) => {
    const key = JSON.stringify(cfg);
    if (key === lastCfg) return;
    lastCfg = key;
    configure(cfg).catch((e) => {
      if (lastCfg === key) lastCfg = '';
      console.warn('overlay_configure failed', e);
    });
  };
  return {
    sync: (s: AppState, prev?: AppState) => send(overlayConfig(s, prev)),
    ready: (s: AppState) => {
      lastCfg = '';
      send(overlayConfig(s));
    },
  };
}

export function applyNativeAction(a: NativeAction) {
  const s = useApp.getState();
  const openReply = () => void reply.open(recentLangs(s.feed));
  const dictate = () => void dictation.toggle(recentLangs(s.feed));
  ({ overlay: s.toggleOverlay, pause: s.togglePause, emergency: s.emergency, session: s.toggleSession, reply: openReply, dictate })[a]();
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

/**
 * Sends settings to the Rust core when they change: `key` makes them
 * comparable (null: nothing to send yet), `apply` runs `ms` after the last
 * change. `stop` also drops a pending send.
 */
export function debouncedSync(key: (s: AppState) => string | null, apply: (s: AppState) => void, ms: number) {
  let lastKey = '';
  let timer: ReturnType<typeof setTimeout> | undefined;
  return {
    push: (s: AppState) => {
      const k = key(s);
      if (k === null || k === lastKey) return;
      lastKey = k;
      clearTimeout(timer);
      timer = setTimeout(() => apply(s), ms);
    },
    stop: () => clearTimeout(timer),
  };
}

/** Settings that the Rust speech service needs, as a comparable key. */
export const speechConfigKey = (s: AppState) => JSON.stringify(speechConfigFrom(s));

/**
 * First run: the installer is small, the app downloads its models by itself
 * (and again when the user picks another Whisper model or turns translation on).
 */
function startSetupBridge(): Promise<() => void>[] {
  const unlisten: Promise<() => void>[] = [];
  unlisten.push(modelDownload.onProgress((status) => useSetup.setState({ status })));
  // First launch: nothing is fetched before the user chose their languages.
  const sync = debouncedSync((s) => (s.langSetupDone ? JSON.stringify(setupNeeds(s)) : null), startSetup, 300);
  sync.push(useApp.getState());
  unlisten.push(Promise.resolve(useApp.subscribe((s) => sync.push(s))));
  unlisten.push(Promise.resolve(sync.stop));
  return unlisten;
}

/** Settings that the Rust translation service needs, as a comparable key. */
export const translationConfigKey = (s: AppState) => JSON.stringify(translationConfigFrom(s));

/** Mirrors the console.log chat: followed while the session captures, lines into the Script. */
function startChatBridge(): Promise<() => void>[] {
  const unlisten: Promise<() => void>[] = [];
  chat.status().then((status) => status && useChat.setState({ status }));
  unlisten.push(chat.onStatus((status) => useChat.setState({ status })));
  unlisten.push(
    chat.onMessage((m) => {
      const app = useApp.getState();
      if (capturesDuring(app.session)) app.addChat(chatEntry(m));
    }),
  );
  const sync = debouncedSync(
    (s) => JSON.stringify(chatConfigFrom(s)),
    (s) => chat.configure(chatConfigFrom(s)).then((status) => status && useChat.setState({ status })),
    400,
  );
  sync.push(useApp.getState());
  if (capturesDuring(useApp.getState().session)) chat.start();
  const unsub = useApp.subscribe((next, prev) => {
    sync.push(next);
    const action = captureActionFor(prev.session, next.session);
    if (action === 'start') chat.start().then((status) => status && useChat.setState({ status }));
    if (action === 'stop') chat.stop();
  });
  unlisten.push(Promise.resolve(unsub));
  unlisten.push(Promise.resolve(sync.stop));
  return unlisten;
}

/** Mirrors the local translation: model states, and each translated line onto its transcript. */
function startTranslationBridge(): Promise<() => void>[] {
  const unlisten: Promise<() => void>[] = [];
  translation.status().then((status) => status && useTranslation.setState({ status }));
  unlisten.push(translation.onStatus((status) => useTranslation.setState({ status })));
  unlisten.push(
    translation.onResult((t) => {
      if (t.partial) return useApp.getState().applyPartialTranslation(t);
      recordTime(t);
      useApp.getState().applyTranslation(t);
    }),
  );
  const sync = debouncedSync(
    translationConfigKey,
    (s) => translation.configure(translationConfigFrom(s)).then((status) => status && useTranslation.setState({ status })),
    250,
  );
  sync.push(useApp.getState());
  unlisten.push(Promise.resolve(useApp.subscribe((s) => sync.push(s))));
  unlisten.push(Promise.resolve(sync.stop));
  return unlisten;
}

/** Online LLM: settings follow the UI (the key is sent only from its card); counters refreshed every 3 s. */
function startLlmBridge(): Promise<() => void>[] {
  const unlisten: Promise<() => void>[] = [];
  const sync = debouncedSync(
    (s) => JSON.stringify(llmConfigFrom(s)),
    (s) => llm.configure(llmConfigFrom(s)).then((status) => status && useLlm.setState({ status })),
    400,
  );
  sync.push(useApp.getState());
  unlisten.push(Promise.resolve(useApp.subscribe((s) => sync.push(s))));
  unlisten.push(Promise.resolve(sync.stop));
  const timer = setInterval(() => {
    if (useApp.getState().llm.enabled) refreshLlm();
  }, 3000);
  unlisten.push(Promise.resolve(() => clearInterval(timer)));
  return unlisten;
}

/** Mirrors the Rust speech recognition: status, real transcripts, voice activity. */
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
      if (capturesDuring(app.session)) app.upsertTranscript(transcriptEntry(t));
    }),
  );
  unlisten.push(speech.onRetract((id) => useApp.getState().retractTranscript(id)));

  // Model, VAD threshold, languages and glossary follow the UI settings.
  const sync = debouncedSync(
    speechConfigKey,
    (s) => speech.configure(speechConfigFrom(s)).then((status) => status && useSpeech.setState({ status })),
    250,
  );
  sync.push(useApp.getState());
  unlisten.push(Promise.resolve(useApp.subscribe((s) => sync.push(s))));
  unlisten.push(Promise.resolve(sync.stop));
  return unlisten;
}

export function startNativeBridge(): () => void {
  if (!isTauri) return () => {};
  const unlisten: Promise<() => void>[] = [];
  unlisten.push(onNativeAction(applyNativeAction));
  unlisten.push(...startAudioBridge());
  unlisten.push(...startSpeechBridge());
  unlisten.push(...startTranslationBridge());
  unlisten.push(...startLlmBridge());
  unlisten.push(...startChatBridge());
  unlisten.push(...startSetupBridge());

  const push = () => overlayWindow.pushState(overlayPayload(useApp.getState()));
  const windowFlags = overlayFlagsSync((cfg) => overlayWindow.configure(cfg));
  unlisten.push(
    overlayWindow.onReady(() => {
      push();
      windowFlags.ready(useApp.getState());
    }),
  );

  const sync = (s: AppState, prev?: AppState) => {
    windowFlags.sync(s, prev);
    if (!prev || prev.feed !== s.feed || prev.fmt !== s.fmt || prev.opacity !== s.opacity || prev.textSize !== s.textSize || prev.maxBlocks !== s.maxBlocks || prev.duration !== s.duration || prev.corner !== s.corner || prev.repos !== s.repos || prev.uiLang !== s.uiLang || prev.target !== s.target || prev.radar !== s.radar || prev.radarDemo !== s.radarDemo) push();
  };
  sync(useApp.getState());
  const unsub = useApp.subscribe(sync);
  // The radar follows the map being played.
  const unsubMap = useGsi.subscribe((g, prev) => g.status?.map !== prev.status?.map && push());

  return () => {
    unsub();
    unsubMap();
    unlisten.forEach((p) => p.then((f) => f()));
  };
}
