// 2 · THE PROBLEM (5–12 s): Russian and English callouts cross in the dark.
import * as THREE from 'three';
import { OX } from './s3-technology.js';
import { cardMaterial, floorMaterial } from '../materials.js';
import { blurred, messageCard, texture } from '../text.js';
import { el, split, reveal, show } from '../dom.js';
import { C, FONT, clamp, easeIn, easeInOut, lerp, range, smooth } from '../util.js';

// Lines from the app's demo data (src/app/state/data.ts).
const MESSAGES = [
  { head: 'VOIX · RUSSE', color: C.ember, text: 'Двое идут на B, двое на B', mono: true, voice: true, p: [-1.2, 1.9, -1.0], dir: 1, v: 0.32 },
  { head: 'CHAT · ÉQUIPE CT · ANGLAIS', color: C.ice, text: 'gonna smoke mid, wait for my flash', p: [5.2, 0.7, -4.2], dir: -1, v: 0.3 },
  { head: 'VOIX · ANGLAIS', color: C.ember, text: 'one low, he is lit, eighty HP', voice: true, p: [-4.5, -0.5, -7.0], dir: 1, v: 0.42 },
  { head: 'CHAT · TOUS · RUSSE', color: C.ice, text: 'гг, хорошая игра', mono: true, p: [4.0, -1.55, 0.4], dir: -1, v: 0.38 },
  { head: 'VOIX · RUSSE', color: C.ember, text: 'Сейвим, не покупайте, эко раунд', mono: true, voice: true, p: [1.5, 3.0, -8.5], dir: -1, v: 0.24 },
  { head: 'CHAT · ÉQUIPE T · ANGLAIS', color: C.ice, text: 'drop AWP pls', p: [8.0, -0.1, -2.2], dir: -1, v: 0.5 },
  { head: 'VOIX · RUSSE', color: C.ember, text: 'Бомба на A, ротейт быстро', mono: true, voice: true, p: [-3.5, -2.1, -3.6], dir: 1, v: 0.36 },
];
const PX_PER_UNIT = 300;
const FOCUS_Z = -1.4;

export default function problem(ctx) {
  const group = new THREE.Group();
  group.position.x = OX; // scenes 1–2 sit where the pipeline (scene 3) starts

  const floor = new THREE.Mesh(new THREE.PlaneGeometry(90, 90), floorMaterial());
  floor.rotation.x = -Math.PI / 2;
  floor.position.y = -3.3;
  group.add(floor);

  const cards = MESSAGES.map((m, i) => {
    const c = messageCard({ ...m, seed: i * 17 + 3 });
    const sharp = texture(c, ctx.renderer);
    const soft = texture(blurred(c, 14), ctx.renderer);
    const mat = cardMaterial(sharp, soft);
    const mesh = new THREE.Mesh(new THREE.PlaneGeometry(c.width / PX_PER_UNIT, c.height / PX_PER_UNIT), mat);
    group.add(mesh);
    return { m, mesh, mat, i };
  });

  const layer = el(ctx.dom, '', { inset: '0' }, 'layer');
  const shade = el(layer, '', { left: '0', top: '0', width: '1100px', height: '1080px', background: 'linear-gradient(90deg, rgba(2,2,3,.78), rgba(2,2,3,.35) 60%, transparent)' });
  const l1 = el(layer, 'Different languages.', { left: '150px', top: '392px', font: `400 116px/1 ${FONT.serif}`, letterSpacing: '-0.02em', color: C.textHi, textShadow: '0 4px 40px rgba(0,0,0,.9)' });
  const l2 = el(layer, 'One team.', { left: '150px', top: '520px', font: `italic 400 116px/1 ${FONT.serif}`, letterSpacing: '-0.02em', color: C.emberHi, textShadow: '0 4px 40px rgba(0,0,0,.9)' });
  const kicker = el(layer, 'COUNTER-STRIKE 2 · VOIX ET CHAT', { left: '154px', top: '350px', font: `500 15px ${FONT.mono}`, letterSpacing: '0.32em', color: C.muted2 });
  const w1 = split(l1, 'word');
  const w2 = split(l2, 'word');

  return {
    name: 'problem',
    from: 5,
    to: 12,
    group,
    layer,
    bg: () => ({ ember: 0.55, ice: 0.6, emberPos: [0.25, 0.6], icePos: [0.85, 0.3], dust: 1 }),
    cam(t) {
      const k = easeInOut(range(t, 4.4, 12.4));
      return {
        pos: new THREE.Vector3(OX + lerp(-0.4, 1.6, k), lerp(0.55, 0.1, k), lerp(9.6, 7.8, k)),
        target: new THREE.Vector3(OX + lerp(0.6, 1.4, k), lerp(0.1, 0, k), -2),
      };
    },
    update(t) {
      const a = smooth(range(t, 4.6, 5.8)) * (1 - smooth(range(t, 11.2, 12.2)));
      floor.material.uniforms.uOpacity.value = a;
      floor.material.uniforms.uTime.value = t;

      const textHold = smooth(range(t, 5.6, 6.4)) * (1 - smooth(range(t, 10.2, 10.9)));
      for (const { m, mesh, mat, i } of cards) {
        const local = t - 5;
        let x = m.p[0] + m.dir * m.v * local;
        let y = m.p[1] + Math.sin(local * 0.6 + i) * 0.08;
        let z = m.p[2];
        // At the end the callouts are drawn into the signal (scene 3 begins there).
        const suck = easeIn(range(t, 10.5 + i * 0.07, 11.7 + i * 0.07));
        x = lerp(x, 1.4, suck);
        y = lerp(y, 0, suck);
        z = lerp(z, -1.5, suck);
        mesh.position.set(x, y, z);
        mesh.scale.setScalar(lerp(1, 0.15, suck));
        mesh.rotation.y = -0.12 * m.dir + Math.sin(local * 0.3 + i) * 0.04;
        const blur = clamp(Math.abs(z - FOCUS_Z) / 5.5);
        mat.uniforms.uBlur.value = blur;
        const depthDim = lerp(1, 0.55, blur);
        // Cards behind the title step back while it is read.
        const behind = x < 2.0 ? 1 - textHold * 0.55 : 1;
        mat.uniforms.uTint.value.setScalar(depthDim * behind);
        mat.uniforms.uOpacity.value = smooth(range(t, 5.0 + i * 0.28, 5.7 + i * 0.28)) * (1 - suck) * (0.55 + 0.45 * (1 - blur));
      }

      show(shade, textHold);
      show(kicker, textHold);
      reveal(w1, t, 5.75, 0.18, 1.1, { y: 22, blur: 12 });
      reveal(w2, t, 7.1, 0.2, 1.2, { y: 22, blur: 12 });
      const out = smooth(range(t, 10.2, 10.9));
      show(l1, 1 - out, { blur: out * 10, x: -out * 30 });
      show(l2, 1 - out, { blur: out * 10, x: -out * 30 });
    },
  };
}
