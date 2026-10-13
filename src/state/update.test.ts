import { describe, expect, it } from 'vitest';
import type { UpdateStatus } from '../backend';
import { PRIVATE_FEED, updateSummary, updateView } from './update';

const latest = { build: 95, version: '0.2.0', commit: 'abc1234', date: '2026-10-13 00:00 UTC', notes: ['feat: réglages conservés', 'fix: délai'], setup: { url: 'https://raw.githubusercontent.com/x', sha256: 'a'.repeat(64), bytes: 15_000_000 } };
const st = (over: Partial<UpdateStatus>): UpdateStatus => ({ state: 'idle', build: 92, version: '0.2.0', commit: 'f2be466', latest: null, doneBytes: 0, totalBytes: 0, error: null, ...over });

describe('in-app update', () => {
  it('offers a newer build with its notes, until dismissed', () => {
    const v = updateView(st({ state: 'available', latest }), null, false);
    expect(v).toMatchObject({ banner: true, action: 'download', title: 'Nouvelle version disponible · build 95' });
    expect(v.detail).toContain('réglages conservés');
    expect(updateView(st({ state: 'available', latest }), 95, false).banner).toBe(false);
    expect(updateView(st({ state: 'uptodate', latest: { ...latest, build: 92 } }), null, false).banner).toBe(false);
  });

  it('shows the download, then waits for the end of the session to install', () => {
    const d = updateView(st({ state: 'downloading', latest, doneBytes: 7_500_000, totalBytes: 15_000_000 }), null, false);
    expect(d.percent).toBe(50);
    expect(d.detail).toContain('SHA-256');
    const busy = updateView(st({ state: 'ready', latest }), null, true);
    expect(busy).toMatchObject({ action: 'install', blocked: true });
    expect(busy.detail).toMatch(/Arrêtez la session/);
    expect(updateView(st({ state: 'ready', latest }), null, false).blocked).toBe(false);
  });

  it('reports errors honestly', () => {
    const v = updateView(st({ state: 'error', latest, error: 'empreinte SHA-256 invalide : installateur refusé' }), null, false);
    expect(v).toMatchObject({ banner: true, action: 'retry', title: 'Mise à jour impossible' });
    expect(updateSummary(st({ state: 'error', error: 'recherche impossible : le serveur a répondu 404' }))).toBe(PRIVATE_FEED);
    expect(updateSummary(st({ state: 'error', error: 'recherche impossible : connexion impossible (Dns)' }))).toContain('connexion');
    expect(updateSummary(st({ state: 'uptodate' }))).toBe('Crosstalk est à jour.');
    expect(updateSummary(null)).toMatch(/version Windows/);
  });
});
