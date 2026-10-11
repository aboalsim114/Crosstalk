import { create } from 'zustand';
import { config } from '../config';
import { isCapturing, useAudio } from './audio';
import { EVENTS, GLOSSARY, type GlossEntry, type ModelId, type PageId, type SimEvent } from './data';

export type Session = 'IDLE' | 'INITIALIZING' | 'LISTENING' | 'DEGRADED' | 'PAUSED' | 'STOPPING' | 'STOPPED';
/** 'transcrit': real speech recognition in the original language, not translated yet. */
export type FeedStatus = 'provisoire' | 'final' | 'original' | 'erreur' | 'transcrit';
export type FeedMode = 'text' | 'voice' | 'mixed';
export type OverlayFormat = 'minimal' | 'bilingue' | 'accessible';
export type Corner = 'tl' | 'tr' | 'bl' | 'br';
export type CaptureSource = 'process' | 'device' | 'wav';
export type Accel = 'cpu' | 'cuda';
export type Engine = 'DeepL' | 'Azure' | 'Local';
export type ToggleKey = 'click' | 'noise' | 'cloud' | 'hist' | 'skipFr';
export type SourceLang = 'ru' | 'en' | 'de' | 'es' | 'pt' | 'pl';

export interface FeedEntry extends SimEvent {
  id: number;
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
  capture: CaptureSource;
  vad: number;
  model: ModelId;
  accel: Accel;
  engine: Engine;
  src: Record<SourceLang, boolean>;
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
  tg: { click: true, noise: false, cloud: false, hist: false, skipFr: true },
  capture: 'process',
  vad: 55,
  model: 'small',
  accel: 'cuda',
  engine: 'DeepL',
  src: { ru: true, en: true, de: false, es: false, pt: false, pl: false },
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

  // Plays the next simulated event: voice goes VAD → ASR (provisional) → final; chat lands at once.
  // In the desktop app voices are really transcribed: only the written chat is still simulated.
  function tick() {
    const s = get();
    if (!isLive(s) || s.paused) return;
    const realVoice = useAudio.getState().native;
    if (realVoice && s.mode === 'voice') {
      schedule(3000);
      return;
    }
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
    toggleSource: (k) => set((s) => ({ src: { ...s.src, [k]: !s.src[k] } })),
    toggleOverlay: () => set((s) => ({ overlayOn: !s.overlayOn })),

    toggleSession: () => {
      const s = get();
      if (isRunning(s)) {
        clearTimers();
        set({ session: 'STOPPING', speaking: false, stage: 0, paused: false });
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
      set({ session: 'STOPPED', paused: false, speaking: false, stage: 0, level: 0 });
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
