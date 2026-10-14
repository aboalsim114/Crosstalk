import { StrictMode } from 'react';
import { act, fireEvent, render } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { DictationState, ReplyTarget } from '../../shared/ipc';

const calls = { toggle: vi.fn(), hide: vi.fn() };
let emitDict: (st: DictationState) => void = () => {};
let emitOpen: (t: ReplyTarget) => void = () => {};

vi.mock('../../shared/ipc', async (orig) => {
  const real = await orig<typeof import('../../shared/ipc')>();
  return {
    ...real,
    reply: {
      ...real.reply,
      phrases: () => Promise.resolve([]),
      models: () => Promise.resolve([]),
      onOpen: (cb: (t: ReplyTarget) => void) => ((emitOpen = cb), Promise.resolve(() => {})),
      hide: () => (calls.hide(), Promise.resolve()),
    },
    dictation: {
      ...real.dictation,
      onState: (cb: (st: DictationState) => void) => ((emitDict = cb), Promise.resolve(() => {})),
      toggle: () => (calls.toggle(), Promise.resolve()),
    },
  };
});

const { ReplyWindow } = await import('./ReplyWindow');

const base: DictationState = { state: 'listening', level: 0.2, seconds: 1, text: null, translated: null, target: 'ru', via: null, copied: false, error: null };
const listening = base;
const copied: DictationState = { ...base, state: 'done', text: 'двое на B', translated: 'двое на B', via: 'model', copied: true };

afterEach(() => {
  vi.useRealTimers();
  calls.toggle.mockReset();
  calls.hide.mockReset();
});

describe('reply pop-up', () => {
  it('Escape stops the dictation once, even in strict mode', () => {
    render(
      <StrictMode>
        <ReplyWindow />
      </StrictMode>,
    );
    act(() => emitDict(listening));
    fireEvent.keyDown(window, { key: 'Escape' });
    expect(calls.toggle).toHaveBeenCalledTimes(1);
    expect(calls.hide).not.toHaveBeenCalled();
  });

  it('reopening the pop-up cancels a pending return to the game', () => {
    vi.useFakeTimers();
    render(<ReplyWindow />);
    act(() => emitDict(copied));
    act(() => emitOpen('en'));
    act(() => void vi.advanceTimersByTime(3000));
    expect(calls.hide).not.toHaveBeenCalled();
  });
});
