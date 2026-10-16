import { beforeEach, describe, expect, it } from 'vitest';
import { act, fireEvent, render, screen } from '@testing-library/react';
import type { UpdateStatus } from '../../shared/ipc';
import { useUpdate } from './update';
import { WhatsNew, readSeen, whatsNew, writeSeen } from './WhatsNew';

const news = [
  { build: 95, date: '2026-10-16', title: 'La voix IA dit vos réponses écrites', items: ['Entrée fait parler la voix IA.'] },
  { build: 94, date: '2026-10-15', title: 'Sous-titres plus fluides', items: ['Modèle plus léger quand le PC est chargé.'] },
];
const latest = { build: 95, version: '0.4.1', commit: 'abc1234', date: '2026-10-16 01:35 UTC', notes: [], news, setup: { url: 'https://raw.githubusercontent.com/x', sha256: 'a'.repeat(64), bytes: 1 } };
const st = (build: number, over: Partial<UpdateStatus> = {}): UpdateStatus => ({ state: 'uptodate', build, version: '0.4.1', commit: 'abc1234', latest, doneBytes: 0, totalBytes: 0, error: null, ...over });

describe('what’s new after an update', () => {
  beforeEach(() => {
    localStorage.clear();
    useUpdate.setState({ status: null });
  });

  it('lists what changed since the version last seen, or only this version the first time', () => {
    expect(whatsNew(st(95), 93)?.map((n) => n.build)).toEqual([95, 94]);
    expect(whatsNew(st(95), 95)).toEqual([]);
    expect(whatsNew(st(95), null)?.map((n) => n.build)).toEqual([95]);
    expect(whatsNew(st(95, { latest: null }), 93)).toBeNull();
    expect(whatsNew(null, 93)).toBeNull();
  });

  it('shows once after an update, then remembers it', async () => {
    writeSeen(93);
    render(<WhatsNew />);
    expect(screen.queryByRole('dialog')).toBeNull();
    await act(async () => useUpdate.setState({ status: st(95) }));
    const dialog = screen.getByRole('dialog', { name: 'Nouveautés' });
    expect(dialog.textContent).toContain('La voix IA dit vos réponses écrites');
    expect(dialog.textContent).toContain('Sous-titres plus fluides');
    expect(dialog.textContent).toContain('VERSION 0.4.1.95');
    expect(dialog.textContent).toContain('16 octobre 2026');
    expect(dialog.textContent).not.toMatch(/abc1234|github|SHA/i);
    await act(async () => fireEvent.click(screen.getByText('C’est parti')));
    expect(screen.queryByRole('dialog')).toBeNull();
    expect(readSeen()).toBe(95);
  });

  it('stays closed when there is nothing new, and remembers the version', async () => {
    writeSeen(95);
    render(<WhatsNew />);
    await act(async () => useUpdate.setState({ status: st(95) }));
    expect(screen.queryByRole('dialog')).toBeNull();
    localStorage.clear();
    // A version without news of its own: nothing shown, version remembered.
    await act(async () => useUpdate.setState({ status: st(97, { latest: { ...latest, build: 97 } }) }));
    expect(screen.queryByRole('dialog')).toBeNull();
  });
});
