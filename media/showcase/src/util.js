// Small deterministic helpers: every value of the film is a pure function of time.

export const clamp = (x, a = 0, b = 1) => Math.min(b, Math.max(a, x));
export const lerp = (a, b, k) => a + (b - a) * k;
/** 0 → 1 as t goes from a to b (clamped). */
export const range = (t, a, b) => clamp((t - a) / (b - a));
export const smooth = (k) => k * k * (3 - 2 * k);
export const easeInOut = (k) => (k < 0.5 ? 4 * k * k * k : 1 - Math.pow(-2 * k + 2, 3) / 2);
export const easeOut = (k) => 1 - Math.pow(1 - k, 3);
export const easeIn = (k) => k * k * k;
export const easeOutExpo = (k) => (k >= 1 ? 1 : 1 - Math.pow(2, -10 * k));
/** The app's --ease-cine: cubic-bezier(0.2, 0.7, 0.1, 1), solved numerically. */
export function cine(k) {
  const x1 = 0.2, y1 = 0.7, x2 = 0.1, y2 = 1;
  const bx = (u) => 3 * x1 * u * (1 - u) ** 2 + 3 * x2 * u * u * (1 - u) + u ** 3;
  const by = (u) => 3 * y1 * u * (1 - u) ** 2 + 3 * y2 * u * u * (1 - u) + u ** 3;
  let lo = 0, hi = 1;
  for (let i = 0; i < 24; i++) {
    const mid = (lo + hi) / 2;
    if (bx(mid) < k) lo = mid; else hi = mid;
  }
  return by((lo + hi) / 2);
}
/** Envelope: fades in over [a, a+fi], holds, fades out over [b-fo, b]. */
export function env(t, a, b, fi = 0.6, fo = 0.6) {
  return smooth(range(t, a, a + fi)) * (1 - smooth(range(t, b - fo, b)));
}
export function mulberry32(seed) {
  let s = seed >>> 0;
  return () => {
    s = (s + 0x6d2b79f5) >>> 0;
    let r = Math.imul(s ^ (s >>> 15), 1 | s);
    r = (r + Math.imul(r ^ (r >>> 7), 61 | r)) ^ r;
    return ((r ^ (r >>> 14)) >>> 0) / 4294967296;
  };
}
export const hexToRgb = (hex) => {
  const n = parseInt(hex.slice(1), 16);
  return [((n >> 16) & 255) / 255, ((n >> 8) & 255) / 255, (n & 255) / 255];
};

export const C = {
  ink: '#020203',
  text: '#ece6dc',
  textHi: '#f3ede3',
  text2: '#bdb6ab',
  text3: '#a49d92',
  muted: '#8f8a83',
  muted2: '#6e6961',
  muted3: '#5a5650',
  muted4: '#4a4640',
  ember: '#ffa24a',
  emberHi: '#ffc68a',
  ice: '#7fd4ff',
  ok: '#7fe0a8',
};
export const FONT = {
  serif: "'Instrument Serif', Georgia, serif",
  sans: "'Geist Sans', system-ui, sans-serif",
  mono: "'JetBrains Mono', ui-monospace, monospace",
};
