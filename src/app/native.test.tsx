import { beforeEach, describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import { applyNativeAction, debouncedSync, overlayConfig, overlayFlagsSync, overlayPayload } from './native';
import { OverlayWindow, visibleLines } from '../windows/overlay/OverlayWindow';
import { useApp, type FeedEntry } from './state/store';
import { transcriptEntry } from '../features/speech/speech';
import { chatEntry } from '../features/chat/chat';

const initial = useApp.getState();
beforeEach(() => useApp.setState(initial, true));

const line = (id: number, over: Partial<FeedEntry> = {}): FeedEntry => ({
  kind: 'voice',
  src: 'ru',
  orig: 'Бомба на A',
  tr: 'Bombe sur A.',
  id,
  time: '14:00:00',
  status: 'final',
  shown: 'Bombe sur A.',
  lat: 1200,
  err: false,
  ...over,
});

describe('overlay window flags', () => {
  it('stays hidden until a session runs', () => {
    const s = useApp.getState();
    expect(overlayConfig(s)).toEqual({ visible: false, clickThrough: true, corner: 'tr' });
    expect(overlayConfig({ ...s, session: 'LISTENING' }).visible).toBe(true);
    expect(overlayConfig({ ...s, session: 'LISTENING', overlayOn: false }).visible).toBe(false);
  });

  it('shows and accepts clicks while repositioning', () => {
    const s = { ...useApp.getState(), repos: true };
    expect(overlayConfig(s)).toMatchObject({ visible: true, clickThrough: false });
  });

  it('only snaps to a corner when the corner changes', () => {
    const s = useApp.getState();
    expect(overlayConfig({ ...s, opacity: 10 }, s).corner).toBeNull();
    expect(overlayConfig({ ...s, corner: 'bl' }, s).corner).toBe('bl');
  });

  it('applies the saved corner even when the overlay window did not exist yet', async () => {
    // Seen in the built app: the first call fails ("overlay window missing")
    // and the overlay stayed top-right although the saved corner was bottom-left.
    const sent: (string | null)[] = [];
    let missing = true;
    const flags = overlayFlagsSync((cfg) => {
      sent.push(cfg.corner);
      return missing ? Promise.reject('overlay window missing') : Promise.resolve();
    });
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => {});
    const saved = { ...useApp.getState(), corner: 'bl' as const };
    flags.sync(saved);
    await Promise.resolve();
    missing = false;
    flags.ready(saved);
    expect(sent).toEqual(['bl', 'bl']);
    // Unchanged flags are not sent twice.
    flags.sync(saved, saved);
    flags.sync(saved, saved);
    expect(sent).toEqual(['bl', 'bl', null]);
    warn.mockRestore();
  });

  it('sends at most three lines to the overlay', () => {
    const feed = [1, 2, 3, 4, 5].map((i) => line(i));
    expect(overlayPayload({ ...useApp.getState(), feed }).feed).toHaveLength(3);
  });
});

describe('native actions', () => {
  it('maps tray / global shortcut actions to the store', () => {
    applyNativeAction('overlay');
    expect(useApp.getState().overlayOn).toBe(false);
    applyNativeAction('session');
    expect(useApp.getState().session).toBe('INITIALIZING');
    applyNativeAction('emergency');
    expect(useApp.getState().session).toBe('STOPPED');
  });
});

describe('overlay window', () => {
  it('fades lines older than the configured duration', () => {
    const now = 100_000;
    expect(visibleLines([line(now - 1000), line(now - 7000)], 6, now)).toHaveLength(1);
  });

  it('shows real lines: their ids are counters, not arrival times', () => {
    // Regression: the overlay read the id as an arrival time, so every real
    // voice or chat line looked decades old and was never shown.
    const now = new Date();
    const voice = transcriptEntry({ id: 3, partial: false, text: 'Двое на B', lang: 'ru', langProb: 0.9, audioMs: 1200, latencyMs: 800, asrMs: 250, model: 'parakeet-tdt-0.6b-v3' }, now);
    const chat = chatEntry({ id: 2 ** 40 + 5, sender: 'Ivan', message: 'гг', lang: 'ru', channel: 'ALL', location: null, dead: false }, now);
    expect(visibleLines([voice, chat], 6, now.getTime() + 1000)).toHaveLength(2);
    expect(visibleLines([voice, chat], 6, now.getTime() + 7000)).toHaveLength(0);
    const base = overlayPayload(useApp.getState());
    render(<OverlayWindow initial={{ ...base, feed: [voice] }} />);
    expect(screen.getByText(/Двое на B/)).toBeInTheDocument();
  });

  it('shows a line again for the full duration when its translation arrives', () => {
    const t0 = Date.now() - 5000;
    const entry = transcriptEntry({ id: 9, partial: false, text: 'Бомба на A', lang: 'ru', langProb: 0.9, audioMs: 1000, latencyMs: 700, asrMs: 200, model: 'parakeet-tdt-0.6b-v3' }, new Date(t0));
    useApp.setState({ feed: [entry] });
    useApp.getState().applyTranslation({ id: 9, lang: 'ru', original: 'Бомба на A', translated: 'Bombe sur A.', error: null, ms: 300, queueMs: 0, cached: false, glossary: false, target: 'fr' });
    const [f] = useApp.getState().feed;
    expect(f.shown).toBe('Bombe sur A.');
    expect(visibleLines([f], 4, Date.now())).toHaveLength(1);
  });

  it('renders nothing when there is no fresh line, and samples while repositioning', () => {
    const base = overlayPayload(useApp.getState());
    const { container, unmount } = render(<OverlayWindow initial={{ ...base, feed: [line(1)] }} />);
    expect(container.textContent).toBe('');
    unmount();
    render(<OverlayWindow initial={{ ...base, repos: true }} />);
    expect(screen.getByText(/Deux joueurs arrivent sur B/)).toBeInTheDocument();
  });

  it('shows a fresh live line', () => {
    const base = overlayPayload(useApp.getState());
    render(<OverlayWindow initial={{ ...base, feed: [line(Date.now())] }} />);
    expect(screen.getByText('« Bombe sur A. »')).toBeInTheDocument();
  });
});

describe('settings sent to the Rust core', () => {
  it('waits for the settings to settle, sends each change once, and stop drops a pending send', () => {
    vi.useFakeTimers();
    try {
      const sent: number[] = [];
      const sync = debouncedSync((s) => (s.textSize > 0 ? String(s.textSize) : null), (s) => sent.push(s.textSize), 250);
      const s = useApp.getState();
      sync.push({ ...s, textSize: 20 });
      sync.push({ ...s, textSize: 22 });
      vi.advanceTimersByTime(249);
      expect(sent).toEqual([]);
      vi.advanceTimersByTime(1);
      expect(sent).toEqual([22]);
      // Same settings: nothing new; no key yet: nothing either.
      sync.push({ ...s, textSize: 22 });
      sync.push({ ...s, textSize: 0 });
      vi.advanceTimersByTime(1000);
      expect(sent).toEqual([22]);
      sync.push({ ...s, textSize: 24 });
      sync.stop();
      vi.advanceTimersByTime(1000);
      expect(sent).toEqual([22]);
    } finally {
      vi.useRealTimers();
    }
  });
});
