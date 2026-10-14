import { beforeEach, describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import { applyNativeAction, debouncedSync, overlayConfig, overlayFlagsSync, overlayPayload } from './native';
import { OverlayWindow, visibleLines } from '../windows/overlay/OverlayWindow';
import { useApp, type FeedEntry } from './state/store';

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
