// 5 · THE EXPERIENCE (33–42 s): a Russian callout, transcribed, then translated in the
// in-game overlay: provisional line first, final line next, then a chat line stacks on top.
// The game is evoked the way the app's overlay preview does it (dark area, diagonal
// stripes, green crosshair): no Counter-Strike 2 asset is used.
import * as THREE from 'three';
import { AppWindow } from '../windows.js';
import { cardMaterial } from '../materials.js';
import { canvas, overlayBlock, quote, textLine, texture } from '../text.js';
import { el, split, reveal, show } from '../dom.js';
import { C, FONT, easeInOut, lerp, mulberry32, range, smooth } from '../util.js';
import { RU, FR } from './s3-technology.js';

export const GAME_W = 12;
export const GAME_H = 6.75;
const OV_SCALE = 3;
const OV_W = 3.5; // world width of a 380 px overlay block (≈1.45× its share of a 1080p screen)

/** The game area: app preview styles, plus light shafts and soft bokeh for depth. */
export function gameCanvas() {
  const w = 2400, h = 1350;
  const { c, ctx } = canvas(w, h);
  const g = ctx.createRadialGradient(w * 0.6, h * 0.4, 0, w * 0.6, h * 0.4, w * 0.62);
  g.addColorStop(0, '#2a2620');
  g.addColorStop(0.55, '#121114');
  g.addColorStop(1, '#08080a');
  ctx.fillStyle = g;
  ctx.fillRect(0, 0, w, h);
  // Light shafts from the top right.
  ctx.save();
  ctx.globalCompositeOperation = 'lighter';
  for (const [x, wd, a] of [[1500, 260, 0.03], [1820, 160, 0.022], [1150, 120, 0.015]]) {
    const lg = ctx.createLinearGradient(0, 0, 0, h);
    lg.addColorStop(0, `rgba(255,198,138,${a})`);
    lg.addColorStop(1, 'rgba(255,198,138,0)');
    ctx.fillStyle = lg;
    ctx.beginPath();
    ctx.moveTo(x, 0);
    ctx.lineTo(x + wd, 0);
    ctx.lineTo(x + wd - 700, h);
    ctx.lineTo(x - 700, h);
    ctx.fill();
  }
  const r = mulberry32(5);
  ctx.filter = 'blur(18px)';
  for (let i = 0; i < 14; i++) {
    ctx.fillStyle = `rgba(255,170,90,${0.03 + r() * 0.05})`;
    ctx.beginPath();
    ctx.arc(r() * w, r() * h * 0.8, 20 + r() * 60, 0, Math.PI * 2);
    ctx.fill();
  }
  ctx.restore();
  // repeating-linear-gradient(135deg, rgba(255,255,255,.018) 0 2px, transparent 2px 22px)
  ctx.save();
  ctx.strokeStyle = 'rgba(255,255,255,0.022)';
  ctx.lineWidth = 4;
  for (let x = -h; x < w + h; x += 44) {
    ctx.beginPath();
    ctx.moveTo(x, 0);
    ctx.lineTo(x + h, h);
    ctx.stroke();
  }
  ctx.restore();
  // Crosshair (rgba(127,224,168,.7)).
  ctx.fillStyle = 'rgba(127,224,168,0.75)';
  ctx.fillRect(w / 2 - 2, h / 2 - 22, 4, 16);
  ctx.fillRect(w / 2 - 2, h / 2 + 6, 4, 16);
  ctx.fillRect(w / 2 - 22, h / 2 - 2, 16, 4);
  ctx.fillRect(w / 2 + 6, h / 2 - 2, 16, 4);
  ctx.font = `500 22px ${FONT.mono}`;
  ctx.letterSpacing = '5px';
  ctx.fillStyle = C.muted4;
  ctx.textAlign = 'center';
  ctx.fillText('CAPTURE DU JEU · FENÊTRÉ SANS BORDURES', w / 2, h - 50);
  return c;
}

export default function experience(ctx) {
  const group = new THREE.Group();
  const gameTex = texture(gameCanvas(), ctx.renderer);
  const game = new AppWindow(gameTex, ctx.renderer, group, { w: GAME_W, h: GAME_H, radius: 0.14, floorY: -3.7 });

  // Voice indicator: redrawn every frame (live bars).
  const vi = canvas(760, 150);
  const viTex = texture(vi.c, ctx.renderer);
  viTex.generateMipmaps = false;
  viTex.minFilter = THREE.LinearFilter;
  const voice = new THREE.Mesh(new THREE.PlaneGeometry(2.4, 2.4 * (150 / 760)), cardMaterial(viTex));
  voice.renderOrder = 11;
  voice.position.set(-GAME_W / 2 + 0.45 + 1.2, -GAME_H / 2 + 0.95, 0.05);
  group.add(voice);

  // The spoken Russian phrase, revealed word by word (mono, as originals are in the app).
  const ruFont = `500 92px ${FONT.mono}`;
  const ru = textLine(RU, ruFont, C.textHi, { pad: 40, glow: 'rgba(255,162,74,0.55)' });
  const ruW = 5.6;
  const ruMesh = new THREE.Mesh(new THREE.PlaneGeometry(ruW, (ruW * ru.c.height) / ru.c.width), cardMaterial(texture(ru.c, ctx.renderer)));
  ruMesh.renderOrder = 11;
  ruMesh.position.set(-2.1, -0.75, 0.9);
  group.add(ruMesh);
  const m = canvas(10, 10).ctx;
  m.font = ruFont;
  const words = RU.split(' ');
  const wordEnds = words.map((_, i) => (ru.pad + m.measureText(words.slice(0, i + 1).join(' ')).width + 6) / ru.c.width);
  const WORD_T = [34.05, 34.4, 34.7, 34.95, 35.3, 35.6, 35.85];
  const tagC = textLine('TRANSCRIPTION · RUSSE', `600 30px ${FONT.mono}`, C.ember, { pad: 10 });
  const tagMesh = new THREE.Mesh(new THREE.PlaneGeometry(1.6, (1.6 * tagC.c.height) / tagC.c.width), cardMaterial(texture(tagC.c, ctx.renderer)));
  tagMesh.renderOrder = 11;
  tagMesh.position.set(-2.1 - ruW / 2 + 0.8 + 0.12, -0.12, 0.9);
  group.add(tagMesh);

  // Overlay blocks (OverlayStack styles), anchored top-right like the app's default corner.
  const mk = (o) => {
    const c = overlayBlock({ ...o, s: OV_SCALE });
    const h = (OV_W * c.height) / c.width;
    const mesh = new THREE.Mesh(new THREE.PlaneGeometry(OV_W, h), cardMaterial(texture(c, ctx.renderer)));
    mesh.renderOrder = 12;
    group.add(mesh);
    return { mesh, h };
  };
  const b1p = mk({ head: 'VOIX • RUSSE → FRANÇAIS (PROVISOIRE)', color: C.ember, orig: RU, line: quote('Deux joueurs arrivent sur…') });
  const b1f = mk({ head: 'VOIX • RUSSE → FRANÇAIS', color: C.ember, orig: RU, line: quote(FR.replace('deux sur B.', 'deux\u00a0sur\u00a0B.')) });
  const b2 = mk({ head: 'CHAT • ÉQUIPE CT • ANGLAIS → FRANÇAIS', color: C.ice, orig: 'gonna smoke mid, wait for my flash', line: 'Player123 : ' + quote('Je vais poser une smoke au mid, attendez ma flash.') });
  const RIGHT = GAME_W / 2 - 0.28, TOP = GAME_H / 2 - 0.28, GAP = (8 / 380) * OV_W;
  // The finale reuses the game area with the last overlay state painted on it.
  {
    const gc = gameCanvas();
    const g = gc.getContext('2d');
    const k = gc.width / GAME_W;
    let y = (GAME_H / 2 - TOP) * k;
    for (const b of [b2, b1f]) {
      const src = b.mesh.material.uniforms.map.value.image;
      const w = OV_W * k, h = b.h * k;
      g.drawImage(src, (GAME_W / 2 + RIGHT) * k - w, y, w, h);
      y += h + GAP * k;
    }
    ctx.gameTex = texture(gc, ctx.renderer);
  }

  const layer = el(ctx.dom, '', { inset: '0' }, 'layer');
  const kicker = el(layer, `EN PARTIE <span style="color:${C.muted2}">· CS2 EN FENÊTRÉ SANS BORDURES</span>`, { left: '150px', top: '92px', font: `500 15px ${FONT.mono}`, letterSpacing: '0.3em', color: C.ember });
  const shade = el(layer, '', { left: '0', bottom: '0', width: '1200px', height: '520px', background: 'radial-gradient(ellipse 80% 90% at 0% 100%, rgba(2,2,3,.85), transparent 70%)' });
  const cap = el(layer, `Lisible <i style="color:${C.emberHi}">en un regard.</i>`, { left: '128px', bottom: '138px', font: `400 84px/1 ${FONT.serif}`, letterSpacing: '-0.015em', color: C.textHi, textShadow: '0 4px 30px rgba(0,0,0,.9)' });
  const capWords = split(cap, 'word');
  const sub = el(layer, 'La traduction s’affiche. Vous restez sur le round.', { left: '132px', bottom: '88px', font: `300 24px ${FONT.sans}`, color: C.text2 });

  const bars = mulberry32(11);
  const barSeeds = Array.from({ length: 28 }, () => bars());
  function drawVoice(t, level) {
    const g = vi.ctx;
    g.clearRect(0, 0, 760, 150);
    g.fillStyle = 'rgba(6,6,8,0.72)';
    g.beginPath();
    g.roundRect(4, 4, 752, 142, 71);
    g.fill();
    g.strokeStyle = 'rgba(255,162,74,0.35)';
    g.lineWidth = 2;
    g.stroke();
    g.fillStyle = C.ember;
    g.beginPath();
    g.arc(74, 75, 14 + level * 6, 0, Math.PI * 2);
    g.globalAlpha = 0.25 + level * 0.3;
    g.fill();
    g.globalAlpha = 1;
    g.beginPath();
    g.arc(74, 75, 10, 0, Math.PI * 2);
    g.fill();
    g.font = `600 30px ${FONT.mono}`;
    g.letterSpacing = '5px';
    g.fillText('VOIX · RU', 116, 86);
    for (let i = 0; i < 28; i++) {
      const v = Math.abs(Math.sin(t * (6 + barSeeds[i] * 9) + i * 0.7)) * (0.35 + 0.65 * barSeeds[(i * 7) % 28]);
      const bh = 8 + v * 90 * level;
      g.fillStyle = i % 5 === 0 ? C.emberHi : C.ember;
      g.globalAlpha = 0.85;
      g.fillRect(380 + i * 12.5, 75 - bh / 2, 6, bh);
    }
    g.globalAlpha = 1;
    viTex.needsUpdate = true;
  }

  return {
    name: 'experience',
    from: 33,
    to: 42,
    blendIn: 0.9,
    group,
    layer,
    bg: () => ({ ember: 0.45, ice: 0.2, emberPos: [0.7, 0.3], icePos: [0.1, 0.9], dust: 0.5, bloom: 0.2, bloomThr: 0.62 }),
    cam(t) {
      const wide = { p: new THREE.Vector3(-1.7, 0.45, 13.4), q: new THREE.Vector3(0.25, 0, 0) };
      const close = { p: new THREE.Vector3(2.75, 1.75, 6.9), q: new THREE.Vector3(3.55, 2.0, 0) };
      const back = { p: new THREE.Vector3(0, 0.35, 12.8), q: new THREE.Vector3(0, 0, 0) };
      const k1 = easeInOut(range(t, 36.0, 38.3));
      const k2 = easeInOut(range(t, 39.7, 41.9));
      const d = Math.sin((t - 33) * 0.35) * 0.25;
      const pos = wide.p.clone().lerp(close.p, k1).lerp(back.p, k2);
      pos.x += d;
      return { pos, target: wide.q.clone().lerp(close.q, k1).lerp(back.q, k2) };
    },
    update(t) {
      const a = smooth(range(t, 32.6, 33.6)) * (1 - smooth(range(t, 41.7, 42.3)));
      const gk = easeInOut(range(t, 32.5, 34.2));
      game.set({ x: 0, y: 0, z: lerp(-5, 0, gk), ry: lerp(-0.25, 0, gk), opacity: a, dim: 1, sweep: lerp(-0.6, 1.9, range(t, 33.4, 35.2)), reflect: 0.6 });

      const speaking = smooth(range(t, 33.8, 34.1)) * (1 - smooth(range(t, 35.9, 36.4)));
      drawVoice(t, speaking);
      voice.material.uniforms.uOpacity.value = a * smooth(range(t, 33.5, 34.0)) * (1 - smooth(range(t, 37.5, 38.2)));

      let rev = 0;
      WORD_T.forEach((wt, i) => {
        const prev = i ? wordEnds[i - 1] : ru.pad / ru.c.width;
        rev = Math.max(rev, lerp(prev, wordEnds[i], easeInOut(range(t, wt - 0.12, wt + 0.18))));
      });
      if (t < WORD_T[0] - 0.12) rev = 0;
      ruMesh.material.uniforms.uReveal.value = rev + 0.04;
      const ruOut = smooth(range(t, 37.2, 38.0));
      ruMesh.material.uniforms.uOpacity.value = a * (1 - ruOut * 0.8) * (1 - smooth(range(t, 39.2, 39.8)));
      ruMesh.position.y = -0.75 - ruOut * 0.15;
      tagMesh.material.uniforms.uOpacity.value = a * smooth(range(t, 33.9, 34.3)) * (1 - smooth(range(t, 37.2, 37.8)));

      // Overlay: provisional at 35.9, final at 37.0, chat line on top at 38.6.
      const in1 = easeInOut(range(t, 35.9, 36.35));
      const fin = smooth(range(t, 36.95, 37.3));
      const in2 = easeInOut(range(t, 38.6, 39.05));
      const push = in2 * (b2.h + GAP);
      const y1 = TOP - b1f.h / 2 - push + (1 - in1) * 0.12;
      b1p.mesh.position.set(RIGHT - OV_W / 2, TOP - b1p.h / 2 - push + (1 - in1) * 0.12, 0.06);
      b1f.mesh.position.set(RIGHT - OV_W / 2, y1, 0.061);
      b1p.mesh.material.uniforms.uOpacity.value = a * in1 * (1 - fin);
      b1f.mesh.material.uniforms.uOpacity.value = a * in1 * fin;
      b2.mesh.position.set(RIGHT - OV_W / 2, TOP - b2.h / 2 + (1 - in2) * 0.12, 0.062);
      b2.mesh.material.uniforms.uOpacity.value = a * in2;

      show(kicker, smooth(range(t, 33.4, 34.0)) * (1 - smooth(range(t, 35.8, 36.4))));
      const capV = smooth(range(t, 37.3, 37.8)) * (1 - smooth(range(t, 40.9, 41.5)));
      show(shade, capV);
      show(cap, capV);
      reveal(capWords, t, 37.35, 0.14, 1.0, { y: 18, blur: 10 });
      show(sub, smooth(range(t, 37.9, 38.6)) * (1 - smooth(range(t, 40.9, 41.5))), { y: (1 - smooth(range(t, 37.9, 38.6))) * 10 });
    },
  };
}
