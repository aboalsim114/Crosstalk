import { describe, expect, it } from 'vitest';
import { css } from './css';
import { size } from './api';
import { PH, PHASES, phaseOf, phaseSub } from './model';

describe('installer phases', () => {
  it('follow the design bounds', () => {
    expect(PH).toEqual([0, 12, 62, 88, 100]);
    expect([0, 11.9, 12, 61, 62, 87, 88, 99].map((p) => phaseOf(p))).toEqual([0, 0, 1, 1, 2, 2, 3, 3]);
    expect(phaseOf(100)).toBe(3);
    expect(phaseOf(40, true)).toBe(3);
  });

  it('show what the installer reports, else the phase text', () => {
    expect(phaseSub(1, 'Reconnaissance vocale (Parakeet TDT 0.6B v3) · 120 / 671 Mo', true)).toBe('Reconnaissance vocale (Parakeet TDT 0.6B v3) · 120 / 671 Mo');
    expect(phaseSub(1, '', true)).toBe(PHASES[1][2]);
    expect(phaseSub(3, '', false)).toBe('Création des raccourcis (menu Démarrer uniquement)');
    expect(phaseSub(3, '', true)).toBe('Création des raccourcis');
  });
});

describe('css()', () => {
  it('turns the design inline styles into React styles', () => {
    expect(
      css("position:absolute;background:radial-gradient(circle 150px at var(--mx,50%) var(--my,50%),rgba(255,236,226,.5),transparent);-webkit-backdrop-filter:blur(4px);font:500 10px 'Geist';--mx:3%"),
    ).toEqual({
      position: 'absolute',
      background: 'radial-gradient(circle 150px at var(--mx,50%) var(--my,50%),rgba(255,236,226,.5),transparent)',
      WebkitBackdropFilter: 'blur(4px)',
      font: "500 10px 'Geist Sans'",
      '--mx': '3%',
    });
    expect(css("font:800 60px/.94 'Archivo'").font).toBe("800 60px/.94 'Archivo Variable'");
  });
});

describe('size()', () => {
  it('writes sizes like the design', () => {
    expect(size(802_000_000)).toBe('802 Mo');
    expect(size(1_200_000_000)).toBe('1,2 Go');
    expect(size(214_000_000_000)).toBe('214 Go');
    expect(size(1_200_000_000_000)).toBe('1,2 To');
    expect(size(10)).toBe('1 Mo');
  });
});
