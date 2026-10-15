// 4 · THE APPLICATION (22–33 s): the real Crosstalk windows, revealed in 3D.
// Opens on the dashboard's big translated line (same sentence as the end of scene 3),
// pulls back, then the next windows come in one by one while the previous ones recede.
import * as THREE from 'three';
import { AppWindow, WIN_W, WIN_H } from '../windows.js';
import { el, split, reveal, show } from '../dom.js';
import { C, FONT, cine, easeInOut, lerp, range, smooth } from '../util.js';

const XOFF = 1.6;
// Focus changes: [start, end] of each transition, and caption windows.
const SHIFTS = [[25.6, 26.5], [27.9, 28.8], [30.3, 31.2]];
const FOCUS_AT = [23.2, 26.2, 28.5, 30.9];
const CAPTIONS = [
  { k: '01 · ACCUEIL', h: 'Le call, traduit<br><i>en direct.</i>', p: 'Voix et chat de vos coéquipiers, dans votre langue.', a: 23.9, b: 25.8 },
  { k: '02 · AUDIO & IA', h: 'Whisper en local,<br><i>GPU ou processeur.</i>', p: 'Le son de cs2.exe seulement. Les modèles restent sur le PC.', a: 26.3, b: 28.1 },
  { k: '03 · OVERLAY', h: 'Discret,<br><i>clics traversants.</i>', p: 'Format, position, opacité et durée : tout se règle.', a: 28.7, b: 30.5 },
  { k: '04 · LANGUES', h: '6 langues de lecture,<br><i>8 de coéquipiers.</i>', p: 'Glossaire Counter-Strike : les callouts restent intacts.', a: 31.1, b: 33.0 },
];
// Where the dashboard shows the translated line (uv of the screenshot).
const QUOTE_UV = [0.44, 0.325];

export function focusAt(t) {
  let f = 0;
  for (const [a, b] of SHIFTS) f += cine(range(t, a, b));
  return f;
}

export default function application(ctx) {
  const group = new THREE.Group();
  const order = ['02-dashboard', '04-audio', '03-overlay', '05-languages'];
  const wins = order.map((n) => new AppWindow(ctx.screens[n], ctx.renderer, group, { floorY: -2.32 }));

  const layer = el(ctx.dom, '', { inset: '0' }, 'layer');
  const shade = el(layer, '', { left: '0', top: '0', width: '760px', height: '1080px', background: 'linear-gradient(90deg, rgba(2,2,3,.92), rgba(2,2,3,.6) 55%, transparent)' });
  const caps = CAPTIONS.map((c) => {
    const box = el(layer, '', { left: '128px', top: '388px', width: '430px' });
    const k = el(box, c.k, { position: 'relative', font: `500 15px ${FONT.mono}`, letterSpacing: '0.3em', color: C.ember });
    k.style.position = 'relative';
    const h = el(box, c.h.replace(/<i>/g, `<i style="color:${C.emberHi}">`), { font: `400 64px/1.02 ${FONT.serif}`, letterSpacing: '-0.015em', color: C.textHi, marginTop: '22px' });
    h.style.position = 'relative';
    const p = el(box, c.p, { font: `300 21px/1.5 ${FONT.sans}`, color: C.text3, marginTop: '22px' });
    p.style.position = 'relative';
    return { c, box, k, h, p, words: split(h, 'word') };
  });

  const qx = XOFF + (QUOTE_UV[0] - 0.5) * WIN_W;
  const qy = (QUOTE_UV[1] - 0.5) * WIN_H;

  return {
    name: 'application',
    from: 22,
    to: 33,
    blendIn: 0.7,
    group,
    layer,
    bg: () => ({ ember: 0.65, ice: 0.3, emberPos: [0.62, 0.55], icePos: [0.1, 0.9], dust: 0.6, bloom: 0.18, bloomThr: 0.62 }),
    cam(t) {
      const k = easeInOut(range(t, 21.9, 24.3));
      const drift = t - 22;
      const far = new THREE.Vector3(0.05 + Math.sin(drift * 0.22) * 0.7, 0.55 + Math.sin(drift * 0.17) * 0.18, 9.7 - Math.sin(drift * 0.12) * 0.3);
      const near = new THREE.Vector3(qx - 0.05, qy, 1.75);
      return {
        pos: near.lerp(far, k),
        target: new THREE.Vector3(qx, qy, 0).lerp(new THREE.Vector3(0.6, -0.05, 0), k),
      };
    },
    update(t) {
      const a = smooth(range(t, 21.65, 22.35)) * (1 - smooth(range(t, 32.5, 33.4)));
      const f = focusAt(t);
      wins.forEach((w, k) => {
        const s = k - f;
        const x = XOFF + (s < 0 ? s * 3.9 : s * 5.6);
        const z = -Math.abs(s) * 2.7;
        const ry = s < 0 ? 0.36 * Math.min(-s, 1.5) : -0.55 * s;
        const op = (s > 0 ? 1 - smooth(range(s, 0.3, 1.0)) : 1 - smooth(range(-s, 2.1, 2.9))) * a;
        const dim = 1 - 0.55 * Math.min(Math.abs(s), 1.2);
        const sweep = lerp(-0.6, 1.9, range(t, FOCUS_AT[k], FOCUS_AT[k] + 1.7));
        w.set({ x, y: 0, z, ry, opacity: op, dim, sweep });
      });
      caps.forEach(({ c, box, words, k, p }) => {
        const v = smooth(range(t, c.a, c.a + 0.5)) * (1 - smooth(range(t, c.b - 0.45, c.b)));
        show(box, v, { y: (1 - smooth(range(t, c.a, c.a + 0.8))) * 14 });
        reveal(words, t, c.a + 0.1, 0.09, 0.9, { y: 16, blur: 8 });
        show(k, smooth(range(t, c.a, c.a + 0.6)));
        show(p, smooth(range(t, c.a + 0.5, c.a + 1.2)));
      });
      show(shade, smooth(range(t, 23.3, 24.1)) * (1 - smooth(range(t, 32.6, 33.3))));
    },
  };
}
