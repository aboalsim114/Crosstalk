// 6 · THE FINALE (42–50 s): everything gathers into one composition, then the logo.
import * as THREE from 'three';
import { AppWindow } from '../windows.js';
import { Wave } from '../wave.js';
import { GAME_W, GAME_H } from './s5-experience.js';
import { el, split, reveal, show } from '../dom.js';
import { C, FONT, cine, easeInOut, easeOut, lerp, range, smooth } from '../util.js';

// Fan composition, from left to right.
const FAN = [
  { n: '05-languages', from: [-16, -1.5, -9], to: [-9.4, 0, -6.6], ry: 0.55, s: 0.78, d: 0.25 },
  { n: '02-dashboard', from: [-12, -2, -4], to: [-5.1, 0, -3.7], ry: 0.32, s: 0.82, d: 0.0 },
  { n: '04-audio', from: [12, -2, -4], to: [5.1, 0, -3.7], ry: -0.32, s: 0.82, d: 0.08 },
  { n: '03-overlay', from: [16, -1.5, -9], to: [9.4, 0, -6.6], ry: -0.55, s: 0.78, d: 0.3 },
];

export default function finale(ctx) {
  const group = new THREE.Group();
  const game = new AppWindow(ctx.gameTex, ctx.renderer, group, { w: GAME_W, h: GAME_H, radius: 0.14, floorY: -2.32 });
  const wins = FAN.map((f) => ({ f, w: new AppWindow(ctx.screens[f.n], ctx.renderer, group, { floorY: -2.32 }) }));

  // The waveform comes back, glued to the camera, and settles under the logo.
  const wave = new Wave({ count: 7, points: 360, width: 0.026 });
  wave.group.position.z = -7;
  wave.group.visible = false;
  ctx.camera.add(wave.group);

  const layer = el(ctx.dom, '', { inset: '0' }, 'layer');
  const logo = el(
    layer,
    `<span>Cross</span><span style="font-style:italic;color:${C.ember}">talk</span>`,
    { left: '0', right: '0', top: '318px', textAlign: 'center', font: `400 200px/1 ${FONT.serif}`, letterSpacing: '-0.02em', color: C.textHi, textShadow: '0 6px 60px rgba(0,0,0,.9)' },
  );
  const logoChars = split(logo);
  const tag = el(layer, `Comprenez le call. <i style="color:${C.emberHi}">Restez dans le jeu.</i>`, {
    left: '0', right: '0', top: '600px', textAlign: 'center', font: `400 54px/1.1 ${FONT.serif}`, color: C.text,
  });
  const tagWords = split(tag, 'word');
  const avail = el(layer, 'AVAILABLE FOR WINDOWS', { left: '0', right: '0', top: '724px', textAlign: 'center', font: `500 19px ${FONT.mono}`, letterSpacing: '0.46em', color: C.ember });
  const availChars = split(avail);
  const os = el(layer, 'Windows 10 / 11 · x64 · traitement local', { left: '0', right: '0', top: '764px', textAlign: 'center', font: `300 19px ${FONT.sans}`, color: C.muted });
  const url = el(layer, 'github.com/aboalsim114/Crosstalk', { left: '0', right: '0', top: '968px', textAlign: 'center', font: `400 17px ${FONT.mono}`, letterSpacing: '0.12em', color: C.muted2 });

  return {
    name: 'finale',
    from: 42,
    to: 50.5,
    blendIn: 0.6,
    post: 0,
    group,
    layer,
    bg: (t) => ({ ember: lerp(0.5, 0.95, smooth(range(t, 45.5, 47))), ice: 0.35, emberPos: [0.5, lerp(0.45, 0.5, smooth(range(t, 45, 47)))], icePos: [0.92, 0.18], dust: lerp(0.8, 0.35, smooth(range(t, 46, 48))), bloom: lerp(0.2, 0.6, smooth(range(t, 45.2, 46))), bloomThr: lerp(0.62, 0.1, smooth(range(t, 45.2, 46))) }),
    cam(t) {
      const k = easeInOut(range(t, 41.6, 44.8));
      const k2 = easeInOut(range(t, 44.8, 50));
      return {
        pos: new THREE.Vector3(Math.sin((t - 42) * 0.25) * lerp(0, 1.2, k), lerp(0.35, 1.5, k) - k2 * 0.3, lerp(12.8, 23.5, k) - k2 * 3.5),
        target: new THREE.Vector3(0, lerp(0, -0.25, k), lerp(0, -3.2, k)),
      };
    },
    update(t) {
      const enter = smooth(range(t, 41.7, 42.3));
      const out = easeInOut(range(t, 45.4, 46.5));
      const gk = cine(range(t, 42.0, 44.4));
      game.set({ x: 0, y: lerp(0, 0.15, gk) + out * 0.9, z: lerp(0, -1.3, gk), s: lerp(1, 0.6, gk), opacity: enter * (1 - out), reflect: lerp(0.6, 1, gk), sweep: lerp(-0.6, 1.9, range(t, 44.3, 45.5)) });
      wins.forEach(({ f, w }) => {
        const k = cine(range(t, 42.1 + f.d, 44.3 + f.d));
        w.set({
          x: lerp(f.from[0], f.to[0], k),
          y: lerp(f.from[1], f.to[1], k) + out * 0.9,
          z: lerp(f.from[2], f.to[2], k),
          ry: f.ry * lerp(1.6, 1, k),
          s: f.s,
          opacity: smooth(range(t, 42.1 + f.d, 43.0 + f.d)) * (1 - out),
          dim: 0.85,
          sweep: lerp(-0.6, 1.9, range(t, 44.0 + f.d * 2, 45.3 + f.d * 2)),
        });
      });

      wave.group.visible = t > 45.5;
      const grow = easeOut(range(t, 45.6, 46.4));
      const flat = easeInOut(range(t, 46.2, 46.95));
      wave.update({
        t: t * 1.3,
        amp: lerp(0.1, 0.9, grow) * (1 - flat * 0.985),
        half: lerp(4.2, 1.6, flat),
        spread: lerp(0.32, 0, flat),
        reveal: grow,
        y: lerp(-0.05, -0.08, flat),
        envW: lerp(2.6, 6, flat),
        fadeTail: flat * 0.85,
        opacity: smooth(range(t, 45.5, 46.0)),
      });

      reveal(logoChars, t, 46.75, 0.055, 1.1, { y: 26, blur: 14 });
      reveal(tagWords, t, 47.45, 0.11, 1.0, { y: 14, blur: 8 });
      reveal(availChars, t, 48.05, 0.022, 0.7, { y: 8, blur: 5 });
      show(os, smooth(range(t, 48.4, 48.9)));
      show(url, smooth(range(t, 48.5, 49.0)));
    },
  };
}
