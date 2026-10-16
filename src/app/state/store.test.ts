import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { GLOSSARY } from './data';
import { isLive, isRunning, sceneFx, useApp } from './store';

const initial = useApp.getState();
const s = () => useApp.getState();

beforeEach(() => {
  vi.useFakeTimers();
  useApp.setState(initial, true);
  sceneFx.length = 0;
});
afterEach(() => {
  s().emergency();
  vi.useRealTimers();
});

describe('session lifecycle', () => {
  it('starts in IDLE and goes INITIALIZING → LISTENING', () => {
    expect(s().session).toBe('IDLE');
    s().toggleSession();
    expect(s().session).toBe('INITIALIZING');
    expect(isRunning(s())).toBe(true);
    vi.advanceTimersByTime(1400);
    expect(s().session).toBe('LISTENING');
    expect(isLive(s())).toBe(true);
    expect(s().startedAt).toBeGreaterThan(0);
    expect(sceneFx).toContain('ember');
  });

  it('stops through STOPPING → STOPPED', () => {
    s().toggleSession();
    vi.advanceTimersByTime(1400);
    s().toggleSession();
    expect(s().session).toBe('STOPPING');
    vi.advanceTimersByTime(500);
    expect(s().session).toBe('STOPPED');
  });

  it('pauses and resumes only while live', () => {
    s().togglePause();
    expect(s().session).toBe('IDLE');
    s().toggleSession();
    vi.advanceTimersByTime(1400);
    s().togglePause();
    expect(s()).toMatchObject({ session: 'PAUSED', paused: true, speaking: false });
    s().togglePause();
    expect(s()).toMatchObject({ session: 'LISTENING', paused: false });
  });

  it('emergency stop clears everything and shows a toast', () => {
    s().toggleSession();
    vi.advanceTimersByTime(1400 + 1000);
    s().emergency();
    expect(s()).toMatchObject({ session: 'STOPPED', speaking: false, stage: 0, level: 0 });
    expect(s().toast).toMatch(/captures arrêtées/);
    const feedLen = s().feed.length;
    vi.advanceTimersByTime(20000);
    expect(s().feed.length).toBe(feedLen);
    expect(s().toast).toBeNull();
  });
});

describe('simulated pipeline', () => {
  it('plays a voice line provisional → final', () => {
    s().toggleSession();
    vi.advanceTimersByTime(1400 + 1000); // first tick: Russian voice line
    expect(s()).toMatchObject({ speaking: true, stage: 1 });
    vi.advanceTimersByTime(1500);
    expect(s().feed[0]).toMatchObject({ kind: 'voice', status: 'provisoire', shown: 'Deux joueurs arrivent sur…' });
    vi.advanceTimersByTime(900);
    expect(s().feed[0]).toMatchObject({ status: 'final', shown: 'Deux joueurs arrivent sur B, deux sur B.' });
    expect(s().feed[0].lat).toBeGreaterThanOrEqual(1150);
    expect(s().seg).toBe(1);
  });

  it('then lands a chat line immediately', () => {
    s().toggleSession();
    vi.advanceTimersByTime(1400 + 1000 + 3100 + 2400);
    expect(s().feed[0]).toMatchObject({ kind: 'chat', ch: 'CT', sender: 'Player123', status: 'final' });
    expect(sceneFx).toContain('ice');
  });

  it('skips voice lines in text mode', () => {
    s().set({ mode: 'text' });
    s().toggleSession();
    vi.advanceTimersByTime(1400 + 1000);
    expect(s().feed[0].kind).toBe('chat');
  });

  it('keeps at most 40 lines', () => {
    s().set({ mode: 'text' });
    s().toggleSession();
    vi.advanceTimersByTime(1400 + 1000 + 3000 * 60);
    expect(s().feed.length).toBe(40);
  });
});

describe('settings', () => {
  it('toggles switches and source languages', () => {
    s().toggle('hist');
    expect(s().tg.hist).toBe(true);
    s().toggleSource('de');
    expect(s().src.de).toBe(true);
  });

  it('adds and removes glossary terms', () => {
    s().set({ newTerm: '  boost ' });
    s().addTerm();
    expect(s().gloss.at(-1)).toEqual(['boost', 'Conserver', 'boost']);
    expect(s().newTerm).toBe('');
    s().set({ newTerm: '   ' });
    s().addTerm();
    expect(s().gloss).toHaveLength(GLOSSARY.length + 1);
    s().removeTerm(0);
    expect(s().gloss[0][0]).toBe('mid');
  });

  it('runs the 10 s audio test to completion', () => {
    s().runTest();
    expect(s().test).toBe(1);
    vi.advanceTimersByTime(250 * 20);
    expect(s()).toMatchObject({ test: 100, testDone: true });
  });

  it('enter hides the intro after its fade', () => {
    s().enter();
    expect(s()).toMatchObject({ entered: true, showIntro: true });
    vi.advanceTimersByTime(1000);
    expect(s().showIntro).toBe(false);
  });
});
