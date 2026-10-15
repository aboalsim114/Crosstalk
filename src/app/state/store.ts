import { create } from 'zustand';
import { LLM_DEFAULT_URL, type LlmConfig, type LlmService, type StreamOpts, type Translation, type VoiceOptions } from '../../shared/ipc';
import { config } from '../config';
import { isCapturing, useAudio } from '../../features/audio/audio';
import type { UiLang } from '../../shared/i18n';
import { EVENTS, GLOSSARY, type GlossEntry, type ModelId, type PageId, type SimEvent } from './data';
import type { PlayerLang, TargetLang } from '../../features/languages/langs';
import { RADAR_DEFAULTS, type RadarOpts } from '../../features/radar/radar';
import { useTargetLang } from '../../shared/i18n';

export type Session = 'IDLE' | 'INITIALIZING' | 'LISTENING' | 'DEGRADED' | 'PAUSED' | 'STOPPING' | 'STOPPED';
/** 'transcrit': real speech recognition in the original language, not (yet) translated. */
export type FeedStatus = 'provisoire' | 'final' | 'original' | 'erreur' | 'transcrit';
export type FeedMode = 'text' | 'voice' | 'mixed';
export type OverlayFormat = 'minimal' | 'bilingue' | 'accessible';
export type Corner = 'tl' | 'tr' | 'bl' | 'br';
export type CaptureSource = 'process' | 'device' | 'wav';
/** Whisper on the processor, the graphics card (Vulkan), or the faster one measured (auto). */
export type Accel = 'cpu' | 'gpu' | 'auto';
export type ToggleKey = 'click' | 'noise' | 'translate' | 'hist' | 'skipFr' | 'ignoreOwn';
/** Mode streamer: subtitles for OBS (src-tauri/src/stream). */
export type { StreamOpts } from '../../shared/ipc';
/** "Qui parle ?": names read on CS2's voice indicator (src-tauri/src/speaker). */
export interface SpeakerOpts {
  enabled: boolean;
  /** Area of CS2's window, in fractions (0..1). */
  region: { x: number; y: number; w: number; h: number };
}
export const SPEAKER_DEFAULTS: SpeakerOpts = { enabled: false, region: { x: 0, y: 0.5, w: 0.25, h: 0.3 } };
/** Online translation by an LLM (Audio & IA): the user's provider, off by default. The key is not here. */
export const VOICE_DEFAULTS: VoiceOptions = { enabled: false, mic: null, voices: {}, monitor: true, volume: 100, micVolume: 100 };

export type LlmOpts = LlmConfig & { service: LlmService };
export const LLM_DEFAULTS: LlmOpts = { enabled: false, service: 'openai', provider: 'openai', baseUrl: LLM_DEFAULT_URL.openai, model: 'gpt-4o-mini' };
export const STREAM_DEFAULTS: StreamOpts = { enabled: false, names: false, original: false, watermark: true, size: 34, max: 3, duration: 8 };

export type SourceLang = PlayerLang;

export interface FeedEntry extends SimEvent {
  id: number;
  /** Id of the real transcript, to attach its translation. */
  tid?: number;
  /** Why a real transcript stays untranslated (model missing, disabled…). */
  note?: string;
  /** Live words of an utterance still being spoken (partial transcript). */
  live?: boolean;
  time: string;
  status: FeedStatus;
  shown: string | null;
  lat: number | null;
  err: boolean;
}

/** Effects consumed by the 3D scene: 'ember' (voice, orange), 'ice' (chat, blue), 'pulse' (speech). */
export type SceneFx = 'ember' | 'ice' | 'pulse';
export const sceneFx: SceneFx[] = [];

export interface AppState {
  page: PageId;
  session: Session;
  paused: boolean;
  feed: FeedEntry[];
  idx: number;
  /** Simulated input level, 0..1. */
  level: number;
  speaking: boolean;
  /** Active pipeline stage: 0 idle, 1 VAD, 2 ASR, 3 translation, 4 overlay. */
  stage: number;
  mode: FeedMode;
  fmt: OverlayFormat;
  corner: Corner;
  opacity: number;
  textSize: number;
  duration: number;
  maxBlocks: number;
  overlayOn: boolean;
  repos: boolean;
  tg: Record<ToggleKey, boolean>;
  /** CS2 folder or console.log given by hand when the detection fails. */
  chatPath: string;
  /** The player's own name, to leave out their chat messages. */
  ownName: string;
  /** Interface language ("auto": the language of Windows). */
  uiLang: UiLang;
  stream: StreamOpts;
  speaker: SpeakerOpts;
  llm: LlmOpts;
  capture: CaptureSource;
  vad: number;
  model: ModelId;
  accel: Accel;
  /** "Focus voix": keep the centred voice chat, turn down game sounds from the sides. */
  voiceFocus: boolean;
  /** Callout radar in the overlay. */
  radar: RadarOpts;
  /** AI voice in the game (Langues › Voix IA). */
  voice: VoiceOptions;
  /** "Essayer": time the sample radar was asked for (not saved). */
  radarDemo: number | null;
  /** The players' languages (Langues › Mes langues). */
  src: Record<SourceLang, boolean>;
  /** The user's language: everything is translated into it. */
  target: TargetLang;
  /** Languages chosen once (first launch asks before anything else). */
  langSetupDone: boolean;
  gloss: GlossEntry[];
  newTerm: string;
  test: number;
  testDone: boolean;
  diag: 'running' | 'done';
  seg: number;
  drop: number;
  toast: string | null;
  dl: Partial<Record<ModelId, number>>;
  entered: boolean;
  showIntro: boolean;
  startedAt: number;
  now: number;
}

export interface AppActions {
  set: (patch: Partial<AppState>) => void;
  setPage: (page: PageId) => void;
  toggle: (k: ToggleKey) => void;
  setStream: (p: Partial<StreamOpts>) => void;
  setSpeaker: (p: Partial<SpeakerOpts>) => void;
  setLlm: (p: Partial<LlmOpts>) => void;
  toggleSession: () => void;
  togglePause: () => void;
  emergency: () => void;
  enter: () => void;
  enterStart: () => void;
  toggleOverlay: () => void;
  toggleSource: (k: SourceLang) => void;
  runTest: () => void;
  runDiag: () => void;
  pickModel: (id: ModelId, installed: boolean) => void;
  addTerm: () => void;
  removeTerm: (i: number) => void;
  clearFeed: () => void;
  /** Adds a real line (speech transcript) to the Script and the overlay. */
  addLine: (entry: FeedEntry) => void;
  /** A real chat message from console.log (not shown in "voices only" mode). */
  addChat: (entry: FeedEntry) => void;
  /** A real transcript: replaces the live line of the same utterance, or is added. */
  upsertTranscript: (entry: FeedEntry) => void;
  /** Quick French of a live line (provisional, until the final translation). */
  applyPartialTranslation: (t: Translation) => void;
  /** Removes the live line of an utterance finally rejected. */
  retractTranscript: (tid: number) => void;
  /** Attaches the French translation (or its failure) to the transcript line. */
  applyTranslation: (t: Translation) => void;
  flash: (msg: string) => void;
}

const initial: AppState = {
  page: config.startPage,
  session: 'IDLE',
  paused: false,
  feed: [],
  idx: 0,
  level: 0,
  speaking: false,
  stage: 0,
  mode: 'mixed',
  fmt: 'bilingue',
  corner: 'tr',
  opacity: 70,
  textSize: 18,
  duration: 6,
  maxBlocks: 3,
  overlayOn: true,
  repos: false,
  tg: { click: true, noise: false, translate: true, hist: false, skipFr: true, ignoreOwn: true },
  chatPath: '',
  ownName: '',
  // French until every screen is translated, then "auto".
  uiLang: 'fr',
  stream: STREAM_DEFAULTS,
  speaker: SPEAKER_DEFAULTS,
  llm: LLM_DEFAULTS,
  capture: 'process',
  vad: 55,
  model: 'base',
  accel: 'auto',
  voiceFocus: false,
  radar: RADAR_DEFAULTS,
  voice: VOICE_DEFAULTS,
  radarDemo: null,
  src: { fr: false, en: true, ru: true, de: false, es: false, uk: false, pl: false, tr: false },
  target: 'fr',
  langSetupDone: false,
  gloss: GLOSSARY,
  newTerm: '',
  test: 0,
  testDone: false,
  diag: 'done',
  seg: 0,
  drop: 0,
  toast: null,
  dl: {},
  entered: config.skipIntro,
  showIntro: !config.skipIntro,
  startedAt: 0,
  now: 0,
};

export const isLive = (s: Pick<AppState, 'session'>) => s.session === 'LISTENING' || s.session === 'DEGRADED';
export const isRunning = (s: Pick<AppState, 'session'>) =>
  isLive(s) || s.session === 'INITIALIZING' || s.session === 'PAUSED';

// Pending simulation timers, cleared on stop / pause / emergency.
let timers: ReturnType<typeof setTimeout>[] = [];
const later = (fn: () => void, ms: number) => timers.push(setTimeout(fn, ms));
const clearTimers = () => {
  timers.forEach(clearTimeout);
  timers = [];
};
let toastTimer: ReturnType<typeof setTimeout> | undefined;

const liveSession = (): Session => (config.win10Fallback ? 'DEGRADED' : 'LISTENING');

export const useApp = create<AppState & AppActions>()((set, get) => {
  const schedule = (ms = 2400) => later(tick, ms);

  // Plays the next simulated event (browser preview only): voice goes VAD → ASR
  // (provisional) → final; chat lands at once. The desktop app shows only real
  // lines: voices from the capture, chat from console.log.
  function tick() {
    const s = get();
    if (!isLive(s) || s.paused) return;
    const realVoice = useAudio.getState().native;
    if (realVoice) return;
    let i = s.idx;
    let e = EVENTS[i % EVENTS.length];
    let guard = 0;
    while ((((s.mode === 'text' || realVoice) && e.kind === 'voice') || (s.mode === 'voice' && e.kind === 'chat')) && guard < 10) {
      i++;
      e = EVENTS[i % EVENTS.length];
      guard++;
    }
    const id = Date.now() + Math.random();
    const time = new Date().toLocaleTimeString('fr-FR');
    const down = config.translatorDown;
    set({ idx: i + 1 });

    if (e.kind === 'voice') {
      set({ speaking: true, stage: 1 });
      later(() => set({ stage: 2 }), 700);
      later(
        () =>
          set((st) => ({
            speaking: false,
            stage: 3,
            feed: [{ ...e, id, time, status: 'provisoire' as const, shown: e.prov ?? null, lat: null, err: false }, ...st.feed].slice(0, 40),
          })),
        1500,
      );
      later(() => {
        sceneFx.push('ember');
        set((st) => ({
          stage: 4,
          seg: st.seg + 1,
          feed: st.feed.map((f) =>
            f.id === id
              ? { ...f, status: down ? 'erreur' : 'final', shown: down ? null : e.tr, err: down, lat: 1150 + Math.round(Math.random() * 1500) }
              : f,
          ),
        }));
      }, 2400);
      later(() => {
        set({ stage: 0 });
        if (Math.random() < 0.3) set((st) => ({ drop: st.drop + 1 }));
        schedule();
      }, 3100);
    } else {
      const fr = e.src === 'fr' && s.tg.skipFr;
      sceneFx.push('ice');
      set((st) => ({
        feed: [
          {
            ...e,
            id,
            time,
            status: fr ? 'original' : down ? 'erreur' : 'final',
            shown: fr || down ? null : e.tr,
            err: down && !fr,
            lat: fr ? 0 : 280 + Math.round(Math.random() * 600),
          } satisfies FeedEntry,
          ...st.feed,
        ].slice(0, 40),
      }));
      schedule(3000);
    }
  }

  return {
    ...initial,
    set: (patch) => set(patch),
    setPage: (page) => set({ page }),
    toggle: (k) => set((s) => ({ tg: { ...s.tg, [k]: !s.tg[k] } })),
    setStream: (p) => set((s) => ({ stream: { ...s.stream, ...p } })),
    setSpeaker: (p) => set((s) => ({ speaker: { ...s.speaker, ...p } })),
    setLlm: (p) => set((s) => ({ llm: { ...s.llm, ...p } })),
    toggleSource: (k) => set((s) => ({ src: { ...s.src, [k]: !s.src[k] } })),
    toggleOverlay: () => set((s) => ({ overlayOn: !s.overlayOn })),

    toggleSession: () => {
      const s = get();
      if (isRunning(s)) {
        clearTimers();
        set((st) => ({ session: 'STOPPING', speaking: false, stage: 0, paused: false, feed: st.feed.filter((f) => !f.live) }));
        later(() => set({ session: 'STOPPED' }), 500);
        return;
      }
      set({ session: 'INITIALIZING', paused: false });
      later(() => {
        sceneFx.push('ember');
        set({ session: liveSession(), startedAt: Date.now(), now: Date.now() });
        schedule(1000);
      }, 1400);
    },

    togglePause: () => {
      const s = get();
      if (!isLive(s) && s.session !== 'PAUSED') return;
      if (s.paused) {
        set({ paused: false, session: liveSession() });
        schedule(800);
      } else {
        clearTimers();
        set({ paused: true, session: 'PAUSED', speaking: false, stage: 0 });
      }
    },

    emergency: () => {
      clearTimers();
      set((st) => ({ session: 'STOPPED', paused: false, speaking: false, stage: 0, level: 0, feed: st.feed.filter((f) => !f.live) }));
      get().flash('Toutes les captures arrêtées · tampons audio purgés');
    },

    enter: () => {
      set({ entered: true });
      setTimeout(() => set({ showIntro: false }), 1000);
    },
    enterStart: () => {
      get().enter();
      setTimeout(() => get().toggleSession(), 900);
    },

    runTest: () => {
      const s = get();
      if (s.test > 0 && s.test < 100) return;
      set({ test: 1, testDone: false });
      const iv = setInterval(() => {
        const n = Math.min(100, get().test + 5);
        if (n >= 100) {
          clearInterval(iv);
          set({ test: 100, testDone: true });
        } else set({ test: n });
      }, 250);
    },

    runDiag: () => {
      set({ diag: 'running' });
      setTimeout(() => set({ diag: 'done' }), 1600);
    },

    pickModel: (id, installed) => {
      const d = get().dl[id];
      if (installed || d === 100) {
        set({ model: id });
        return;
      }
      if (d) return;
      const iv = setInterval(() => {
        const n = Math.min(100, (get().dl[id] ?? 0) + 7);
        if (n >= 100) clearInterval(iv);
        set((st) => ({ dl: { ...st.dl, [id]: n } }));
      }, 180);
    },

    addTerm: () => {
      const v = get().newTerm.trim();
      if (v) set((st) => ({ gloss: [...st.gloss, [v, 'Conserver', v]], newTerm: '' }));
    },
    removeTerm: (i) => set((st) => ({ gloss: st.gloss.filter((_, j) => j !== i) })),
    clearFeed: () => set({ feed: [] }),
    addLine: (entry) => {
      if (get().mode === 'text') return;
      sceneFx.push('ember');
      set((st) => ({ feed: [entry, ...st.feed].slice(0, 40), seg: st.seg + 1 }));
    },
    addChat: (entry) => {
      if (get().mode === 'voice') return;
      sceneFx.push('ice');
      set((st) => ({ feed: [entry, ...st.feed].slice(0, 40) }));
    },
    upsertTranscript: (entry) => {
      const i = get().feed.findIndex((f) => f.tid === entry.tid && f.live);
      if (i < 0) return get().addLine(entry);
      // The provisional French stays shown until the final translation arrives.
      set((st) => ({ feed: st.feed.map((f, k) => (k === i ? { ...entry, id: f.id, shown: f.shown } : f)) }));
    },
    applyPartialTranslation: (t) =>
      set((st) => ({ feed: st.feed.map((f) => (f.tid === t.id && f.live && t.translated ? { ...f, shown: t.translated } : f)) })),
    retractTranscript: (tid) => set((st) => ({ feed: st.feed.filter((f) => !(f.tid === tid && f.live)) })),
    applyTranslation: (t) =>
      set((st) => ({ feed: st.feed.map((f) => (f.tid === t.id && f.status === 'transcrit' && !f.live ? withTranslation(f, t) : f)) })),

    flash: (msg) => {
      clearTimeout(toastTimer);
      set({ toast: msg });
      toastTimer = setTimeout(() => set({ toast: null }), 2600);
    },
  };
});

/**
 * Drives the session timecode and, without a real capture, the simulated input
 * meter (~11 Hz, like the prototype). During a real capture the level comes from
 * the Rust audio events instead (see native.ts).
 */
export function startMeter(): () => void {
  const iv = setInterval(() => {
    const s = useApp.getState();
    const run = isLive(s) && !s.paused;
    if (isCapturing(useAudio.getState().status)) {
      if (run) useApp.setState({ now: Date.now() });
      return;
    }
    if (!run && s.level === 0) return;
    const r = Math.random();
    const v = !run ? 0 : s.speaking ? 0.45 + r * 0.55 : r < 0.07 ? 0.5 + Math.random() * 0.35 : r * 0.14;
    useApp.setState({ level: v, now: Date.now() });
  }, 90);
  return () => clearInterval(iv);
}

/** Untranslated on purpose: the original stays, with the reason, not as an error. */
const QUIET_FAILURES = ['traduction désactivée', 'modèle de traduction absent', 'langue non gérée'];

/** A transcript line once its translation arrived: French text, or the original with why. */
/** A translation identical to the original: only Counter-Strike terms. */
export const KEPT_NOTE = 'Termes CS conservés tels quels (glossaire, modifiable dans Langues & glossaire)';

export function withTranslation(f: FeedEntry, t: Translation): FeedEntry {
  const total = (f.lat ?? 0) + t.queueMs + t.ms;
  if (t.translated) {
    if (t.lang === (t.target || 'fr')) return { ...f, status: 'original', shown: null, tr: null };
    // Only CS terms kept by the glossary: say so, else it looks untranslated.
    const same = (x: string) => x.toLowerCase().replace(/[^\p{L}\p{N}]+/gu, ' ').trim();
    const note = same(t.translated) === same(f.orig) ? KEPT_NOTE : undefined;
    return { ...f, status: 'final', tr: t.translated, shown: t.translated, lat: total, err: false, note };
  }
  const why = t.error ?? 'moteur indisponible';
  if (QUIET_FAILURES.includes(why)) return { ...f, note: why, shown: null };
  return { ...f, status: 'erreur', shown: null, err: true, lat: total, note: why };
}

// Labels ("→ FRANÇAIS") follow the language of the translations.
useApp.subscribe((s, prev) => {
  if (s.target !== prev.target) useTargetLang.setState({ target: s.target });
});
