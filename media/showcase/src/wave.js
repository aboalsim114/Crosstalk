// The luminous waveform: the visual thread of the film (intro, pipeline, finale).
import * as THREE from 'three';
import { ribbonMaterial } from './materials.js';
import { C } from './util.js';

/** A glowing strip through N points; rebuilt on the CPU every frame. */
export class Ribbon {
  constructor(n, width) {
    this.n = n;
    this.width = width;
    const g = new THREE.BufferGeometry();
    this.pos = new Float32Array(n * 2 * 3);
    this.col = new Float32Array(n * 2 * 3);
    this.alpha = new Float32Array(n * 2);
    const side = new Float32Array(n * 2);
    const idx = [];
    for (let i = 0; i < n; i++) {
      side[i * 2] = -1;
      side[i * 2 + 1] = 1;
      if (i < n - 1) {
        const a = i * 2;
        idx.push(a, a + 1, a + 2, a + 1, a + 3, a + 2);
      }
    }
    g.setAttribute('position', new THREE.BufferAttribute(this.pos, 3));
    g.setAttribute('aColor', new THREE.BufferAttribute(this.col, 3));
    g.setAttribute('aAlpha', new THREE.BufferAttribute(this.alpha, 1));
    g.setAttribute('aSide', new THREE.BufferAttribute(side, 1));
    g.setIndex(idx);
    this.geometry = g;
    this.material = ribbonMaterial();
    this.mesh = new THREE.Mesh(g, this.material);
    this.mesh.frustumCulled = false;
    this.pts = new Float32Array(n * 3);
  }
  /** fn(i, u, out) fills out = [x, y, z, r, g, b, a] for point i (u in 0..1). */
  update(fn) {
    const { n, pts, pos, col, alpha } = this;
    const o = [0, 0, 0, 0, 0, 0, 0];
    for (let i = 0; i < n; i++) {
      fn(i, i / (n - 1), o);
      pts[i * 3] = o[0]; pts[i * 3 + 1] = o[1]; pts[i * 3 + 2] = o[2];
      for (let s = 0; s < 2; s++) {
        col[(i * 2 + s) * 3] = o[3]; col[(i * 2 + s) * 3 + 1] = o[4]; col[(i * 2 + s) * 3 + 2] = o[5];
        alpha[i * 2 + s] = o[6];
      }
    }
    const hw = this.width / 2;
    for (let i = 0; i < n; i++) {
      const a = Math.max(0, i - 1), b = Math.min(n - 1, i + 1);
      let tx = pts[b * 3] - pts[a * 3], ty = pts[b * 3 + 1] - pts[a * 3 + 1];
      const l = Math.hypot(tx, ty) || 1;
      tx /= l; ty /= l;
      const nx = -ty * hw, ny = tx * hw;
      pos[i * 6] = pts[i * 3] - nx; pos[i * 6 + 1] = pts[i * 3 + 1] - ny; pos[i * 6 + 2] = pts[i * 3 + 2];
      pos[i * 6 + 3] = pts[i * 3] + nx; pos[i * 6 + 4] = pts[i * 3 + 1] + ny; pos[i * 6 + 5] = pts[i * 3 + 2];
    }
    this.geometry.attributes.position.needsUpdate = true;
    this.geometry.attributes.aColor.needsUpdate = true;
    this.geometry.attributes.aAlpha.needsUpdate = true;
  }
}

const E = new THREE.Color(C.ember), EH = new THREE.Color(C.emberHi), ICE = new THREE.Color(C.ice);
/** Cover gradient: ember → light ember → ice, across u. */
export function gradient(u, out) {
  const c = u < 0.5 ? E.clone().lerp(EH, u * 2) : EH.clone().lerp(ICE, (u - 0.5) * 2);
  out[3] = c.r; out[4] = c.g; out[5] = c.b;
}

/** Speech-like signal: syllable envelopes times a carrier, deterministic in (x, t). */
export function speech(x, t, k) {
  const syll = Math.max(0, Math.sin(x * 2.1 + t * 1.7 + k)) ** 2 * 0.7 + 0.3;
  return (
    Math.sin(x * 5.3 + t * 3.1 + k * 0.7) * 0.55 +
    Math.sin(x * 11.7 - t * 4.3 + k * 1.3) * 0.25 +
    Math.sin(x * 23.1 + t * 6.7 + k * 2.1) * 0.12
  ) * syll;
}

/** Seven layered ribbons. update() takes the whole shape state. */
export class Wave {
  constructor({ count = 7, points = 360, width = 0.03 } = {}) {
    this.group = new THREE.Group();
    this.ribbons = [];
    for (let i = 0; i < count; i++) {
      const r = new Ribbon(points, i === Math.floor(count / 2) ? width * 1.6 : width);
      this.ribbons.push(r);
      this.group.add(r.mesh);
    }
  }
  /**
   * t: time · amp: amplitude · half: half-length in x · spread: z spacing ·
   * reveal: 0..1 growth from the centre · y: vertical offset · opacity ·
   * fadeTail: 1 = underline look (ember fading to the right like the README cover).
   */
  update({ t, amp = 1, half = 6, spread = 0.35, reveal = 1, y = 0, opacity = 1, fadeTail = 0, envW = 3.2 }) {
    const n = this.ribbons.length;
    this.ribbons.forEach((r, k) => {
      const c = k - (n - 1) / 2;
      const ka = 1 - Math.abs(c) * 0.13;
      r.material.uniforms.uOpacity.value = opacity * (k === (n - 1) / 2 ? 1 : 0.55);
      r.update((i, u, o) => {
        const x = (u - 0.5) * 2 * half;
        const xn = (u - 0.5) * 2; // -1..1
        const envl = Math.exp(-((x / envW) ** 2));
        o[0] = x;
        o[1] = y + amp * ka * envl * speech(x * (6 / half), t, k * 0.45) * 1.15;
        o[2] = c * spread;
        gradient(u, o);
        const edge = Math.abs(xn);
        const rv = 1 - Math.min(1, Math.max(0, (edge - reveal) / 0.08 + 1));
        const tail = fadeTail > 0 ? 1 - fadeTail * u : 1;
        o[6] = rv * (0.25 + 0.75 * (1 - edge * edge)) * tail;
      });
    });
  }
}
