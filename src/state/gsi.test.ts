import { describe, expect, it } from 'vitest';
import type { GsiStatus } from '../backend';
import { gsiSummary } from './gsi';

const st = (over: Partial<GsiStatus>): GsiStatus => ({ installed: true, cfgPath: 'cfg', cs2Found: true, listening: true, port: 31982, error: null, map: null, mapName: null, mode: null, callouts: 0, lastSeen: null, ...over });

describe('map being played (Game State Integration)', () => {
  it('guides the user until CS2 sends the map', () => {
    expect(gsiSummary(st({ cs2Found: false })).value).toBe('CS2 introuvable');
    expect(gsiSummary(st({ installed: false })).value).toBe('Désactivée');
    expect(gsiSummary(st({})).detail).toMatch(/Relancez CS2/);
    expect(gsiSummary(st({ lastSeen: 1 })).value).toMatch(/menus/);
    expect(gsiSummary(st({ error: 'port local 31982 indisponible' })).value).toBe('Indisponible');
  });

  it('shows the map and how many callouts are used', () => {
    const s = gsiSummary(st({ map: 'de_mirage', mapName: 'Mirage', callouts: 23, lastSeen: 1 }));
    expect(s.value).toBe('Carte : Mirage');
    expect(s.detail).toMatch(/^23 callouts de Mirage/);
    expect(gsiSummary(st({ map: 'cs_office', mapName: 'cs_office', lastSeen: 1 })).detail).toMatch(/vocabulaire CS général/);
  });
});
