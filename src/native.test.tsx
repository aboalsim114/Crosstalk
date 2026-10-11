import { beforeEach, describe, expect, it } from 'vitest';
import { render, screen } from '@testing-library/react';
import { applyNativeAction, overlayConfig, overlayPayload } from './native';
import { OverlayWindow, visibleLines } from './overlay/OverlayWindow';
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
