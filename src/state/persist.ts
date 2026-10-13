// Settings kept between launches: %APPDATA%\Crosstalk\config.json and
// glossary.json in the desktop app (src-tauri/src/settings.rs), localStorage
// in the browser preview. Versioned: older files are migrated, unknown or
// invalid values are ignored (the default stays).

import { isTauri, settingsStore } from '../backend';
import { useAudio } from './audio';
import type { GlossEntry, ModelId } from './data';
import { useApp, type AppState } from './store';

export const SETTINGS_VERSION = 1;
const LOCAL_KEY = 'crosstalk.settings';

type Persisted = Pick<
  AppState,
  'mode' | 'fmt' | 'corner' | 'opacity' | 'textSize' | 'duration' | 'maxBlocks' | 'overlayOn' | 'tg' | 'chatPath' | 'ownName' | 'capture' | 'vad' | 'model' | 'accel' | 'src' | 'stream'
>;

const oneOf =
  <T extends string>(...values: T[]) =>
  (v: unknown): v is T =>
    typeof v === 'string' && (values as string[]).includes(v);
const inRange = (lo: number, hi: number) => (v: unknown): v is number => typeof v === 'number' && Number.isFinite(v) && v >= lo && v <= hi;
const isString = (v: unknown): v is string => typeof v === 'string' && v.length <= 500;
const isBool = (v: unknown): v is boolean => typeof v === 'boolean';

const CHECKS: { [K in keyof Persisted]: (v: unknown) => boolean } = {
  mode: oneOf('text', 'voice', 'mixed'),
  fmt: oneOf('minimal', 'bilingue', 'accessible'),
  corner: oneOf('tl', 'tr', 'bl', 'br'),
  opacity: inRange(0, 100),
  textSize: inRange(14, 28),
  duration: inRange(4, 8),
  maxBlocks: inRange(2, 3),
  overlayOn: isBool,
  tg: (v) => typeof v === 'object' && v !== null,
  chatPath: isString,
  ownName: isString,
  capture: oneOf('process', 'device', 'wav'),
  vad: inRange(0, 100),
  model: oneOf<ModelId>('tiny', 'base', 'small', 'medium', 'large-v3-turbo'),
  accel: oneOf('cpu', 'cuda'),
  src: (v) => typeof v === 'object' && v !== null,
  stream: (v) => typeof v === 'object' && v !== null,
};

export function snapshot(s: AppState, deviceId: string | null) {
  const settings = Object.fromEntries((Object.keys(CHECKS) as (keyof Persisted)[]).map((k) => [k, s[k]]));
  return {
    config: { version: SETTINGS_VERSION, settings: { ...settings, deviceId } },
    glossary: { version: SETTINGS_VERSION, terms: s.gloss },
  };
}

/** Older files to the current version (version 0: settings at the top level). */
function migrate(raw: unknown): Record<string, unknown> | null {
  if (typeof raw !== 'object' || raw === null) return null;
  const r = raw as Record<string, unknown>;
  const version = typeof r.version === 'number' ? r.version : 0;
  if (version > SETTINGS_VERSION) return null; // written by a newer Crosstalk: keep the defaults
  const settings = version === 0 ? r : r.settings;
  return typeof settings === 'object' && settings !== null ? (settings as Record<string, unknown>) : null;
}

/** Valid stored values only, merged over the defaults for nested records. */
export function restore(raw: unknown, defaults: AppState): { app: Partial<AppState>; deviceId?: string | null } {
  const s = migrate(raw);
  if (!s) return { app: {} };
  const app: Partial<AppState> = {};
  for (const k of Object.keys(CHECKS) as (keyof Persisted)[]) {
    const v = s[k];
    if (!CHECKS[k](v)) continue;
    if (k === 'stream') {
      // Same type as the default only; the Rust side bounds the numbers again.
      const base = defaults.stream as unknown as Record<string, unknown>;
      const merged: Record<string, unknown> = { ...base };
      for (const [key, val] of Object.entries(v as Record<string, unknown>)) if (key in base && typeof val === typeof base[key]) merged[key] = val;
      app.stream = merged as unknown as AppState['stream'];
    } else if (k === 'tg' || k === 'src') {
      const base = defaults[k] as Record<string, boolean>;
      const merged = { ...base };
      for (const [key, val] of Object.entries(v as Record<string, unknown>)) if (key in base && isBool(val)) merged[key] = val;
      (app as Record<string, unknown>)[k] = merged;
    } else (app as Record<string, unknown>)[k] = v;
  }
  const deviceId = s.deviceId === null || isString(s.deviceId) ? (s.deviceId as string | null) : undefined;
  return { app, deviceId };
}

/** The stored glossary; old default entries are migrated ("CT / T" was one term). */
export function restoreGlossary(raw: unknown): GlossEntry[] | null {
  if (typeof raw !== 'object' || raw === null) return null;
  const terms = (raw as { terms?: unknown }).terms;
  if (!Array.isArray(terms)) return null;
  const out: GlossEntry[] = [];
  for (const t of terms) {
    if (!Array.isArray(t) || t.length !== 3 || !t.every(isString) || !t[0].trim()) continue;
    if (t[1] !== 'Conserver' && t[1] !== 'Traduire') continue;
    if (t[0] === 'CT / T') {
      out.push(['CT', 'Conserver', 'CT'], ['T', 'Conserver', 'T']);
      continue;
    }
    out.push([t[0], t[1], t[2]]);
  }
  return out;
}

/** Loads the stored settings into the stores; call before the first render. */
export async function loadSettings(): Promise<string[]> {
  let stored: { config: unknown; glossary: unknown; warnings: string[] } | null = null;
  try {
    if (isTauri) stored = await settingsStore.load();
    else {
      const local = window.localStorage?.getItem(LOCAL_KEY);
      if (local) stored = { ...(JSON.parse(local) as { config: unknown; glossary: unknown }), warnings: [] };
    }
  } catch {
    stored = null;
  }
  if (!stored) return [];
  const defaults = useApp.getState();
  const { app, deviceId } = restore(stored.config, defaults);
  const gloss = restoreGlossary(stored.glossary);
  useApp.setState({ ...app, ...(gloss ? { gloss } : {}) });
  if (deviceId !== undefined) useAudio.setState({ deviceId });
  return stored.warnings;
}

/** Saves the settings (debounced) whenever they change; returns the unsubscribe. */
export function startSettingsSync(delayMs = 600): () => void {
  let last = JSON.stringify(snapshot(useApp.getState(), useAudio.getState().deviceId));
  let timer: ReturnType<typeof setTimeout> | undefined;
  const check = () => {
    const snap = snapshot(useApp.getState(), useAudio.getState().deviceId);
    const key = JSON.stringify(snap);
    if (key === last) return;
    last = key;
    clearTimeout(timer);
    timer = setTimeout(() => {
      if (isTauri) settingsStore.save(snap.config, snap.glossary).catch((e) => console.warn('settings_save failed', e));
      else {
        try {
          window.localStorage?.setItem(LOCAL_KEY, key);
        } catch {
          /* private browsing: settings stay for this session */
        }
      }
    }, delayMs);
  };
  const a = useApp.subscribe(check);
  const b = useAudio.subscribe(check);
  return () => {
    a();
    b();
    clearTimeout(timer);
  };
}

