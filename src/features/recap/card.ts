// The recap card, drawn on a canvas (1200 × 675, the size Discord and X show
// best) in the app's cinematic style: warm black, film grain, orange for
// voice, serif titles. Exported as PNG.

import type { RecapFacts } from './recap';
import { lanes, type MapLayout } from '../radar/layouts';

export const CARD_W = 1200;
export const CARD_H = 675;

const SERIF = "'Instrument Serif', Georgia, serif";
const SANS = "'Geist Sans', 'Geist', system-ui, sans-serif";
const MONO = "'JetBrains Mono', ui-monospace, monospace";
const ORANGE = '#ffa24a';
const TEXT = '#f4efe6';
const DIM = '#8f8a83';
const FAINT = '#5a5650';

export const REPO_URL = 'github.com/aboalsim114/Crosstalk';

/** Fonts must be loaded before drawing text on a canvas. */
export async function loadCardFonts(): Promise<void> {
  if (typeof document === 'undefined' || !document.fonts) return;
  await Promise.all(
    [`400 120px ${SERIF}`, `italic 400 30px ${SERIF}`, `400 24px ${SANS}`, `500 24px ${SANS}`, `500 16px ${MONO}`].map((f) => document.fonts.load(f).catch(() => [])),
  );
}

/** Shrinks the font until `text` fits in `max` pixels. */
function fit(ctx: CanvasRenderingContext2D, text: string, font: (px: number) => string, px: number, max: number, min = 12): number {
  let size = px;
  ctx.font = font(size);
  while (size > min && ctx.measureText(text).width > max) {
    size -= 2;
    ctx.font = font(size);
  }
  return size;
}

function grain(ctx: CanvasRenderingContext2D, seed: number) {
  // Deterministic noise (same card, same image).
  let x = seed || 1;
  const rnd = () => ((x = (x * 1103515245 + 12345) & 0x7fffffff) / 0x7fffffff);
  for (let i = 0; i < 9000; i++) {
    ctx.fillStyle = `rgba(255,240,220,${rnd() * 0.035})`;
    ctx.fillRect(rnd() * CARD_W, rnd() * CARD_H, 1.4, 1.4);
  }
}

/** Callouts of the match on the map plan (zone id → lines naming it). */
export interface CardHeat {
  layout: MapLayout;
  zones: Record<string, number>;
  /** The most called zones, shown next to the plan. */
  top: { label: string; count: number }[];
}

export interface CardOptions {
  quotes?: string[];
  heat?: CardHeat;
  /** Date line, e.g. "13 octobre 2026". */
  date?: string;
}

/** The map plan with a glow on every zone called during the match, `size` pixels square at (x, y). */
export function drawHeatPlan(ctx: CanvasRenderingContext2D, heat: CardHeat, x: number, y: number, size: number): void {
  const pad = size * 0.08;
  const px = (v: number) => x + pad + v * (size - 2 * pad);
  const py = (v: number) => y + pad + v * (size - 2 * pad);
  const max = Math.max(1, ...Object.values(heat.zones));
  ctx.save();
  ctx.fillStyle = 'rgba(255,255,255,0.025)';
  ctx.strokeStyle = 'rgba(255,255,255,0.08)';
  ctx.lineWidth = 1.5;
  ctx.beginPath();
  ctx.roundRect(x, y, size, size, 18);
  ctx.fill();
  ctx.stroke();
  ctx.strokeStyle = 'rgba(255,255,255,0.09)';
  ctx.lineWidth = 1.6;
  ctx.lineCap = 'round';
  for (const [p, q] of lanes(heat.layout)) {
    ctx.beginPath();
    ctx.moveTo(px(p.x), py(p.y));
    ctx.lineTo(px(q.x), py(q.y));
    ctx.stroke();
  }
  // Glow: area grows with the square root of the count, so that one zone does not hide the others.
  ctx.globalCompositeOperation = 'lighter';
  for (const z of heat.layout.zones) {
    const n = heat.zones[z.id] ?? 0;
    if (!n) continue;
    const k = n / max;
    const r = size * (0.05 + 0.1 * Math.sqrt(k));
    const g = ctx.createRadialGradient(px(z.x), py(z.y), 0, px(z.x), py(z.y), r);
    g.addColorStop(0, `rgba(255,150,60,${0.35 + 0.45 * k})`);
    g.addColorStop(1, 'rgba(255,150,60,0)');
    ctx.fillStyle = g;
    ctx.beginPath();
    ctx.arc(px(z.x), py(z.y), r, 0, Math.PI * 2);
    ctx.fill();
  }
  ctx.globalCompositeOperation = 'source-over';
  for (const z of heat.layout.zones) {
    const n = heat.zones[z.id] ?? 0;
    const site = z.id === 'a' || z.id === 'b';
    if (site) {
      ctx.strokeStyle = n ? ORANGE : 'rgba(255,198,138,0.45)';
      ctx.lineWidth = 1.6;
      ctx.beginPath();
      ctx.arc(px(z.x), py(z.y), 13, 0, Math.PI * 2);
      ctx.stroke();
      ctx.font = `600 15px ${MONO}`;
      ctx.fillStyle = n ? '#fff' : 'rgba(255,198,138,0.75)';
      ctx.textAlign = 'center';
      ctx.fillText(z.label, px(z.x), py(z.y) + 5);
      ctx.textAlign = 'left';
      continue;
    }
    if (z.id === 't' || z.id === 'ct') {
      ctx.font = `600 13px ${MONO}`;
      ctx.fillStyle = n ? '#fff' : z.id === 't' ? 'rgba(255,138,122,0.6)' : 'rgba(127,212,255,0.6)';
      ctx.textAlign = 'center';
      ctx.fillText(z.label, px(z.x), py(z.y) + 4);
      ctx.textAlign = 'left';
      continue;
    }
    ctx.fillStyle = n ? '#fff3e4' : 'rgba(236,230,220,0.35)';
    ctx.beginPath();
    ctx.arc(px(z.x), py(z.y), n ? 3.6 : 2.4, 0, Math.PI * 2);
    ctx.fill();
  }
  // Names of the most called zones, on the plan.
  const named = new Set(heat.top.map((t) => t.label));
  ctx.font = `500 15px ${SANS}`;
  ctx.textAlign = 'center';
  ctx.lineJoin = 'round';
  for (const z of heat.layout.zones) {
    if (!named.has(z.label) || z.id === 'a' || z.id === 'b') continue;
    const ly = z.y < 0.12 ? py(z.y) + 24 : py(z.y) - 12;
    ctx.strokeStyle = 'rgba(0,0,0,0.85)';
    ctx.lineWidth = 4;
    ctx.strokeText(z.label, px(z.x), ly);
    ctx.fillStyle = '#fff';
    ctx.fillText(z.label, px(z.x), ly);
  }
  ctx.restore();
}

export function drawRecapCard(ctx: CanvasRenderingContext2D, f: RecapFacts, opt: CardOptions = {}): void {
  ctx.save();
  ctx.textBaseline = 'alphabetic';
  // Background: warm black with an orange glow behind the big number.
  ctx.fillStyle = '#0d0b09';
  ctx.fillRect(0, 0, CARD_W, CARD_H);
  const glow = ctx.createRadialGradient(250, 420, 20, 250, 420, 520);
  glow.addColorStop(0, 'rgba(255,140,50,0.22)');
  glow.addColorStop(1, 'rgba(255,140,50,0)');
  ctx.fillStyle = glow;
  ctx.fillRect(0, 0, CARD_W, CARD_H);
  const cold = ctx.createRadialGradient(1100, 80, 10, 1100, 80, 420);
  cold.addColorStop(0, 'rgba(127,212,255,0.08)');
  cold.addColorStop(1, 'rgba(127,212,255,0)');
  ctx.fillStyle = cold;
  ctx.fillRect(0, 0, CARD_W, CARD_H);
  grain(ctx, f.hero * 7919 + f.title.length);
  ctx.strokeStyle = 'rgba(255,255,255,0.07)';
  ctx.lineWidth = 2;
  ctx.strokeRect(1, 1, CARD_W - 2, CARD_H - 2);

  // Header.
  ctx.fillStyle = ORANGE;
  ctx.beginPath();
  ctx.arc(66, 66, 6, 0, Math.PI * 2);
  ctx.fill();
  ctx.font = `500 16px ${MONO}`;
  ctx.fillStyle = DIM;
  ctx.fillText('CROSSTALK  ·  RÉSUMÉ DE PARTIE', 86, 72);
  if (opt.date) {
    ctx.textAlign = 'right';
    ctx.fillStyle = FAINT;
    ctx.fillText(opt.date.toUpperCase(), CARD_W - 60, 72);
    ctx.textAlign = 'left';
  }

  // Map and match line.
  ctx.fillStyle = TEXT;
  fit(ctx, f.title, (px) => `400 ${px}px ${SERIF}`, 104, 640, 48);
  ctx.fillText(f.title, 56, 178);
  ctx.font = `400 24px ${SANS}`;
  ctx.fillStyle = DIM;
  ctx.fillText(f.subtitle, 60, 222);

  // The big number.
  ctx.fillStyle = ORANGE;
  fit(ctx, String(f.hero), (px) => `400 ${px}px ${SERIF}`, 230, 520, 80);
  ctx.fillText(String(f.hero), 52, 440);
  ctx.font = `500 26px ${SANS}`;
  ctx.fillStyle = TEXT;
  ctx.fillText(f.heroLabel, 60, 486);

  // Stats column; with the heat map, the plan comes first and three rows below it.
  const x0 = 720;
  const x1 = CARD_W - 60;
  const heat = opt.heat && Object.values(opt.heat.zones).some((n) => n > 0) ? opt.heat : null;
  let y = 290;
  let rows = f.rows;
  if (heat) {
    const size = 300;
    drawHeatPlan(ctx, heat, x0, 100, size);
    const lx = x0 + size + 22;
    ctx.font = `500 13px ${MONO}`;
    ctx.fillStyle = FAINT;
    ctx.fillText('ZONES', lx, 122);
    ctx.fillText('ANNONCÉES', lx, 140);
    let ty = 184;
    for (const t of heat.top.slice(0, 3)) {
      ctx.fillStyle = TEXT;
      fit(ctx, t.label, (px) => `500 ${px}px ${SANS}`, 20, x1 - lx, 12);
      ctx.fillText(t.label, lx, ty);
      ctx.font = `500 28px ${MONO}`;
      ctx.fillStyle = ORANGE;
      ctx.fillText(String(t.count), lx, ty + 32);
      ty += 82;
    }
    y = 458;
    rows = f.rows.slice(0, 3);
  }
  ctx.strokeStyle = 'rgba(255,255,255,0.08)';
  ctx.lineWidth = 1;
  for (const r of rows) {
    ctx.font = `400 21px ${SANS}`;
    ctx.fillStyle = DIM;
    ctx.fillText(r.label, x0, y);
    ctx.font = `500 26px ${MONO}`;
    ctx.fillStyle = TEXT;
    ctx.textAlign = 'right';
    ctx.fillText(r.value, x1, y);
    ctx.textAlign = 'left';
    ctx.beginPath();
    ctx.moveTo(x0, y + 18);
    ctx.lineTo(x1, y + 18);
    ctx.stroke();
    y += 52;
  }

  // Optional quotes (translations only, never names).
  const quotes = opt.quotes ?? [];
  if (quotes.length) {
    let qy = 540;
    ctx.fillStyle = '#d9d2c6';
    for (const q of quotes.slice(0, 2)) {
      const text = `« ${q} »`;
      fit(ctx, text, (px) => `italic 400 ${px}px ${SERIF}`, 30, 620, 18);
      ctx.fillText(text, 60, qy);
      qy += 38;
    }
  }

  // Footer: what Crosstalk is, and where to get it.
  ctx.fillStyle = 'rgba(255,255,255,0.06)';
  ctx.fillRect(0, CARD_H - 70, CARD_W, 70);
  ctx.font = `500 18px ${MONO}`;
  const urlW = ctx.measureText(REPO_URL).width;
  const tagline = 'Voix et chat de CS2 traduits dans votre langue, 100 % en local · gratuit · open source';
  ctx.fillStyle = DIM;
  // Kept clear of the address on the right.
  fit(ctx, tagline, (px) => `400 ${px}px ${SANS}`, 19, CARD_W - 120 - urlW - 32, 13);
  ctx.fillText(tagline, 60, CARD_H - 28);
  ctx.font = `500 18px ${MONO}`;
  ctx.fillStyle = ORANGE;
  ctx.textAlign = 'right';
  ctx.fillText(REPO_URL, CARD_W - 60, CARD_H - 28);
  ctx.restore();
}

/** Draws the card and returns it as PNG bytes. */
export async function renderRecapPng(f: RecapFacts, opt: CardOptions = {}): Promise<Blob> {
  await loadCardFonts();
  const canvas = document.createElement('canvas');
  canvas.width = CARD_W;
  canvas.height = CARD_H;
  const ctx = canvas.getContext('2d');
  if (!ctx) throw new Error('canvas indisponible');
  drawRecapCard(ctx, f, opt);
  return new Promise((resolve, reject) => canvas.toBlob((b) => (b ? resolve(b) : reject(new Error('image impossible'))), 'image/png'));
}
