// 1 · THE INTRO (0–5 s): a signal in the dark becomes the Crosstalk wordmark.
import * as THREE from 'three';
import { OX } from './s3-technology.js';
import { Wave } from '../wave.js';
import { el, split, reveal, show } from '../dom.js';
import { C, FONT, easeInOut, easeOut, lerp, range, smooth } from '../util.js';

export default function intro(ctx) {
  const group = new THREE.Group();
  group.position.x = OX; // scenes 1–2 sit where the pipeline (scene 3) starts
  const wave = new Wave({ count: 7, points: 420, width: 0.028 });
  group.add(wave.group);

  const layer = el(ctx.dom, '', { inset: '0' }, 'layer');
  const logo = el(
    layer,
    `<span>Cross</span><span style="font-style:italic;color:${C.ember}">talk</span>`,
    { left: '0', right: '0', top: '390px', textAlign: 'center', font: `400 200px/1 ${FONT.serif}`, letterSpacing: '-0.02em', color: C.textHi, textShadow: '0 6px 60px rgba(0,0,0,.9)' },
  );
  const logoChars = split(logo);
  const tagline = el(layer, 'EVERY VOICE. ONE LANGUAGE.', {
    left: '0', right: '0', top: '668px', textAlign: 'center', font: `500 22px ${FONT.mono}`, letterSpacing: '0.42em', color: C.text2,
  });
  const tagChars = split(tagline);

  return {
    name: 'intro',
    from: 0,
    to: 5,
    pre: 0,
    group,
    layer,
    bg: (t) => ({ ember: lerp(0, 0.9, smooth(range(t, 0.8, 4))), ice: lerp(0, 0.5, smooth(range(t, 1.5, 4.5))), emberPos: [0.5, 0.5], icePos: [0.92, 0.2], dust: 1 }),
    cam(t) {
      const k = easeOut(range(t, 0, 3.6));
      const k2 = easeInOut(range(t, 3.6, 5.6));
      return {
        pos: new THREE.Vector3(OX + lerp(1.6, 0, k), lerp(0.9, 0, k), lerp(16, 7, k) - k2 * 1.4),
        target: new THREE.Vector3(OX + lerp(0.4, 0, k), 0, 0),
      };
    },
    update(t) {
      // The signal grows from the centre, then flattens into the logo's underline.
      const grow = easeOut(range(t, 0.5, 2.6));
      const flat = easeInOut(range(t, 2.6, 3.7));
      const shrink = easeInOut(range(t, 3.1, 4.1));
      wave.update({
        t: t * 1.3,
        amp: lerp(0.15, 1.25, grow) * (1 - flat * 0.985),
        half: lerp(6.5, 1.6, shrink),
        spread: lerp(0.38, 0, flat),
        reveal: grow,
        y: lerp(0, -0.3, shrink),
        envW: lerp(3.4, 6, shrink),
        fadeTail: shrink * 0.85,
        opacity: smooth(range(t, 0.4, 1.4)) * (1 - smooth(range(t, 4.4, 5.1))),
      });
      wave.group.rotation.y = lerp(-0.35, 0, easeOut(range(t, 0, 3.4)));

      reveal(logoChars, t, 3.0, 0.055, 1.1, { y: 26, blur: 14 });
      reveal(tagChars, t, 3.75, 0.022, 0.8, { y: 10, blur: 6 });
      const out = smooth(range(t, 4.45, 5.15));
      show(logo, 1 - out, { blur: out * 10, scale: 1 + out * 0.08 });
      show(tagline, 1 - out, { blur: out * 6, scale: 1 + out * 0.05 });
    },
  };
}
