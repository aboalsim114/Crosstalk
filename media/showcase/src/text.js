// Canvas drawings turned into textures: message cards, overlay blocks (styles of the
// app's OverlayStack), station labels, and text sampled into particle targets.
import * as THREE from 'three';
import { C, FONT } from './util.js';

export const QO = '« ';
export const QC = ' »';
export const quote = (s) => QO + s + QC;

export function canvas(w, h) {
  const c = document.createElement('canvas');
  c.width = Math.ceil(w);
  c.height = Math.ceil(h);
  return { c, ctx: c.getContext('2d') };
}

export function texture(c, renderer) {
  const t = new THREE.CanvasTexture(c);
  t.colorSpace = THREE.SRGBColorSpace;
  t.anisotropy = renderer ? renderer.capabilities.getMaxAnisotropy() : 4;
  t.generateMipmaps = true;
  t.minFilter = THREE.LinearMipmapLinearFilter;
  t.needsUpdate = true;
  return t;
}

export function blurred(src, px) {
  const { c, ctx } = canvas(src.width, src.height);
  ctx.filter = `blur(${px}px)`;
  ctx.drawImage(src, 0, 0);
  return c;
}

export function roundRect(ctx, x, y, w, h, r) {
  ctx.beginPath();
  ctx.moveTo(x + r, y);
  ctx.arcTo(x + w, y, x + w, y + h, r);
  ctx.arcTo(x + w, y + h, x, y + h, r);
  ctx.arcTo(x, y + h, x, y, r);
  ctx.arcTo(x, y, x + w, y, r);
  ctx.closePath();
}

function wrap(ctx, text, maxW) {
  const words = text.split(' ');
  const lines = [];
  let cur = '';
  for (const w of words) {
    const next = cur ? cur + ' ' + w : w;
    if (ctx.measureText(next).width > maxW && cur) {
      lines.push(cur);
      cur = w;
    } else cur = next;
  }
  if (cur) lines.push(cur);
  return lines;
}

/** Floating message of scene 2: a glass card with a language tag and the original line. */
export function messageCard({ head, color, text, mono, voice, seed = 1 }) {
  const S = 2;
  const font = mono ? `400 ${30 * S}px ${FONT.mono}` : `400 ${32 * S}px ${FONT.sans}`;
  const m = canvas(10, 10).ctx;
  m.font = font;
  const tw = m.measureText(text).width;
  const w = Math.max(tw + 64 * S, 420 * S);
  const h = 128 * S;
  const { c, ctx } = canvas(w + 40, h + 40);
  ctx.translate(20, 20);
  roundRect(ctx, 0, 0, w, h, 18 * S);
  ctx.fillStyle = 'rgba(12,11,13,0.78)';
  ctx.fill();
  ctx.strokeStyle = 'rgba(255,255,255,0.09)';
  ctx.lineWidth = 1.5 * S;
  ctx.stroke();
  ctx.font = `600 ${12.5 * S}px ${FONT.mono}`;
  ctx.letterSpacing = `${2.2 * S}px`;
  ctx.fillStyle = color;
  ctx.fillText(head, 32 * S, 40 * S);
  if (voice) {
    // A small live waveform next to voice lines.
    let r = seed;
    for (let i = 0; i < 16; i++) {
      r = (r * 9301 + 49297) % 233280;
      const bh = (6 + (r / 233280) * 20) * S;
      ctx.fillStyle = color;
      ctx.globalAlpha = 0.75;
      ctx.fillRect(w - (32 + (16 - i) * 7) * S, 34 * S - bh / 2, 3 * S, bh);
    }
    ctx.globalAlpha = 1;
  }
  ctx.letterSpacing = '0px';
  ctx.font = font;
  ctx.fillStyle = C.textHi;
  ctx.fillText(text, 32 * S, 92 * S);
  return c;
}

/**
 * One block of the in-game overlay, drawn with the styles of OverlayStack
 * (src/features/overlay/OverlayPage.tsx), format "Bilingue", opacity 70 %, text 18 px.
 * `s` is the scale factor (1 = CSS pixels of the app).
 */
export function overlayBlock({ head, color, orig, line, s = 3 }) {
  const W = 380 * s;
  const padX = 15 * s;
  const padY = 11 * s;
  const m = canvas(10, 10).ctx;
  m.font = `500 ${18 * s}px ${FONT.sans}`;
  const lines = wrap(m, line, W - padX * 2);
  const lh = 18 * 1.32 * s;
  const H = padY * 2 + 9.5 * s * 1.25 + 5 * s + 11.5 * 1.4 * s + 5 * s + lines.length * lh;
  const { c, ctx } = canvas(W + 8 * s, H + 8 * s);
  ctx.translate(4 * s, 4 * s);
  roundRect(ctx, 0, 0, W, H, 10 * s);
  ctx.fillStyle = 'rgba(6,6,8,0.70)';
  ctx.fill();
  ctx.strokeStyle = 'rgba(255,255,255,0.05)';
  ctx.lineWidth = s;
  ctx.stroke();
  let y = padY + 9.5 * s;
  ctx.font = `600 ${9.5 * s}px ${FONT.mono}`;
  ctx.letterSpacing = `${9.5 * 0.14 * s}px`;
  ctx.fillStyle = color;
  ctx.fillText(head, padX, y);
  ctx.letterSpacing = '0px';
  y += 5 * s + 11.5 * 1.4 * s - 3 * s;
  ctx.font = `400 ${11.5 * s}px ${FONT.mono}`;
  ctx.fillStyle = C.text3;
  ctx.shadowColor = '#000';
  ctx.shadowBlur = 2 * s;
  ctx.shadowOffsetY = s;
  ctx.fillText(orig, padX, y);
  y += 5 * s + lh - 4 * s;
  ctx.font = `500 ${18 * s}px ${FONT.sans}`;
  ctx.fillStyle = '#fff';
  for (const l of lines) {
    ctx.shadowBlur = 10 * s;
    ctx.shadowOffsetY = 0;
    ctx.shadowColor = 'rgba(0,0,0,.85)';
    ctx.fillText(l, padX, y);
    ctx.shadowBlur = 2 * s;
    ctx.shadowOffsetY = s;
    ctx.shadowColor = '#000';
    ctx.fillText(l, padX, y);
    y += lh;
  }
  return c;
}

/** Station label of scene 3: number, serif title, mono detail. */
export function stationLabel({ num, title, sub }) {
  const S = 2;
  const { c, ctx } = canvas(760 * S, 200 * S);
  ctx.font = `500 ${15 * S}px ${FONT.mono}`;
  ctx.letterSpacing = `${4 * S}px`;
  ctx.fillStyle = C.ember;
  ctx.fillText(num, 4 * S, 30 * S);
  ctx.letterSpacing = '0px';
  ctx.font = `400 ${72 * S}px ${FONT.serif}`;
  ctx.fillStyle = C.textHi;
  ctx.fillText(title, 0, 110 * S);
  ctx.font = `400 ${17 * S}px ${FONT.mono}`;
  ctx.letterSpacing = `${1.5 * S}px`;
  ctx.fillStyle = C.muted;
  ctx.fillText(sub, 4 * S, 156 * S);
  return c;
}

/** A single line of text, tight canvas (used for the spoken Russian phrase). */
export function textLine(text, font, color, { pad = 20, glow } = {}) {
  const m = canvas(10, 10).ctx;
  m.font = font;
  const mt = m.measureText(text);
  const h = Math.ceil((mt.actualBoundingBoxAscent + mt.actualBoundingBoxDescent) * 1.6) + pad * 2;
  const { c, ctx } = canvas(mt.width + pad * 2, h);
  ctx.font = font;
  ctx.textBaseline = 'middle';
  if (glow) {
    ctx.shadowColor = glow;
    ctx.shadowBlur = 24;
  }
  ctx.fillStyle = color;
  ctx.fillText(text, pad, h / 2);
  return { c, width: mt.width, height: h, pad };
}

/**
 * Samples `count` points on the glyphs of `text`, centred, `width` world units wide.
 * Returns Float32Array [x0, y0, x1, y1, ...]; deterministic for a given rng.
 */
export function textPoints(text, font, width, count, rng) {
  const m = canvas(10, 10).ctx;
  m.font = font;
  const mt = m.measureText(text);
  const tw = Math.ceil(mt.width) + 20;
  const th = Math.ceil((mt.actualBoundingBoxAscent + mt.actualBoundingBoxDescent) * 1.3) + 20;
  const { c, ctx } = canvas(tw, th);
  ctx.font = font;
  ctx.fillStyle = '#fff';
  ctx.textBaseline = 'middle';
  ctx.fillText(text, 10, th / 2);
  const data = ctx.getImageData(0, 0, tw, th).data;
  const filled = [];
  for (let y = 0; y < th; y++) for (let x = 0; x < tw; x++) if (data[(y * tw + x) * 4 + 3] > 140) filled.push(x, y);
  const n = filled.length / 2;
  const out = new Float32Array(count * 2);
  const k = width / tw;
  for (let i = 0; i < count; i++) {
    const j = Math.floor(rng() * n);
    out[i * 2] = (filled[j * 2] + rng() - 0.5 - tw / 2) * k;
    out[i * 2 + 1] = -(filled[j * 2 + 1] + rng() - 0.5 - th / 2) * k;
  }
  return out;
}
