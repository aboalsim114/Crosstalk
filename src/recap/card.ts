// The recap card, drawn on a canvas (1200 × 675, the size Discord and X show
// best) in the app's cinematic style: warm black, film grain, orange for
// voice, serif titles. Exported as PNG.

import type { RecapFacts } from '../state/recap';

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

export interface CardOptions {
  quotes?: string[];
  /** Date line, e.g. "13 octobre 2026". */
  date?: string;
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

  // Stats column.
  const x0 = 720;
  const x1 = CARD_W - 60;
  let y = 290;
  ctx.strokeStyle = 'rgba(255,255,255,0.08)';
  ctx.lineWidth = 1;
  for (const r of f.rows) {
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
  ctx.font = `400 19px ${SANS}`;
  ctx.fillStyle = DIM;
  ctx.fillText('Voix et chat de CS2 traduits en français, 100 % en local · gratuit · open source', 60, CARD_H - 28);
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
