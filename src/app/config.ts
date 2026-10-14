import { NAV, type PageId } from './state/data';

/**
 * Simulation switches (the prototype's "Tweaks" panel). Read from the URL query so they
 * work in both `npm run dev` and the Tauri dev window, e.g.
 *   http://localhost:1420/?skipIntro&startPage=audio&win10Fallback
 */
export interface SimConfig {
  skipIntro: boolean;
  startPage: PageId;
  autoStart: boolean;
  /** Simulates Windows builds without process loopback (falls back to device loopback). */
  win10Fallback: boolean;
  /** Simulates the translation engine timing out. */
  translatorDown: boolean;
}

function readConfig(): SimConfig {
  const q = new URLSearchParams(typeof location === 'undefined' ? '' : location.search);
  const flag = (k: string) => q.has(k) && q.get(k) !== 'false' && q.get(k) !== '0';
  const page = q.get('startPage');
  return {
    skipIntro: flag('skipIntro'),
    startPage: NAV.some(([id]) => id === page) ? (page as PageId) : 'dash',
    autoStart: flag('autoStart'),
    win10Fallback: flag('win10Fallback'),
    translatorDown: flag('translatorDown'),
  };
}

export const config: SimConfig = readConfig();
