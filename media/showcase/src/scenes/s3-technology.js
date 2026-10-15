// 3 · THE TECHNOLOGY (12–22 s): a tracking shot through the real pipeline of the app.
// Capture (WASAPI, cs2.exe) → Silero VAD → whisper.cpp → LLM (cloud ou local) → overlay.
// The particle signal travels with the camera and changes shape at each station:
// waveform → speech isolated → Russian characters → French sentence → overlay block.
import * as THREE from 'three';
import { pointsMaterial, cardMaterial } from '../materials.js';
import { Ribbon, speech } from '../wave.js';
import { canvas, overlayBlock, quote, roundRect, stationLabel, texture, textPoints } from '../text.js';
import { el, show } from '../dom.js';
import { C, FONT, easeInOut, lerp, mulberry32, range, smooth } from '../util.js';

export const RU = 'Двое идут на B, двое на B';
export const FR = 'Deux joueurs arrivent sur B, deux sur B.';
const N = 9000;
const SPACING = 9;
const X = (i) => (i - 4) * SPACING; // station 5 ends at the origin, where scene 4 begins
export const OX = X(0);
const T = (i) => 12.6 + i * 2.0; // arrival at each station
const MOVE = 1.0;

const STATIONS = [
  { num: '01', title: 'Capture audio', sub: 'WASAPI · SON DE CS2.EXE UNIQUEMENT', stage: ['Capture', 'cs2.exe'] },
  { num: '02', title: 'Détection vocale', sub: 'SILERO VAD · REPÈRE LA PAROLE', stage: ['VAD', 'silero'] },
  { num: '03', title: 'Transcription', sub: 'WHISPER.CPP · CARTE GRAPHIQUE OU PROCESSEUR', stage: ['Transcription', 'whisper.cpp'] },
  { num: '04', title: 'Traduction', sub: 'LLM · CLOUD OU LOCAL', stage: ['Traduction', 'LLM → fr'] },
  { num: '05', title: 'Sous-titres', sub: "L'OVERLAY, PAR-DESSUS LE JEU", stage: ['Overlay', 'bilingue'] },
];

export function camX(t) {
  let x = X(0);
  for (let i = 1; i < 5; i++) x = lerp(x, X(i), easeInOut(range(t, T(i) - MOVE, T(i))));
  return x;
}

function frameTexture(renderer) {
  const S = 2, w = 1700 * S, h = 900 * S;
  const { c, ctx } = canvas(w, h);
  roundRect(ctx, 6, 6, w - 12, h - 12, 28 * S);
  ctx.strokeStyle = 'rgba(243,237,227,0.10)';
  ctx.lineWidth = 2 * S;
  ctx.stroke();
  ctx.strokeStyle = C.ember;
  ctx.globalAlpha = 0.7;
  ctx.lineWidth = 3 * S;
  const L = 46 * S, o = 6;
  for (const [x, y, dx, dy] of [[o, o, 1, 1], [w - o, o, -1, 1], [o, h - o, 1, -1], [w - o, h - o, -1, -1]]) {
    ctx.beginPath();
    ctx.moveTo(x + dx * L, y);
    ctx.lineTo(x + dx * 14 * S, y);
    ctx.quadraticCurveTo(x, y, x, y + dy * 14 * S);
    ctx.lineTo(x, y + dy * L);
    ctx.stroke();
  }
  return texture(c, renderer);
}

function tag(text, color, renderer, size = 26) {
  const S = 2;
  const m = canvas(10, 10).ctx;
  m.font = `600 ${size * S}px ${FONT.mono}`;
  m.letterSpacing = `${3 * S}px`;
  const w = m.measureText(text).width + 20;
  const { c, ctx } = canvas(w, size * S * 1.8);
  ctx.font = m.font;
  ctx.letterSpacing = m.letterSpacing;
  ctx.fillStyle = color;
  ctx.textBaseline = 'middle';
  ctx.fillText(text, 10, c.height / 2);
  return { tex: texture(c, renderer), w: c.width, h: c.height };
}

export default function technology(ctx) {
  const group = new THREE.Group();
  const rng = mulberry32(2024);

  // Stations: a glass frame with corner ticks and a label, every SPACING units.
  const frameTex = frameTexture(ctx.renderer);
  const stations = STATIONS.map((s, i) => {
    const frame = new THREE.Mesh(new THREE.PlaneGeometry(8.5, 4.5), cardMaterial(frameTex));
    frame.position.set(X(i), 0.15, -0.9);
    const lc = stationLabel(s);
    const label = new THREE.Mesh(new THREE.PlaneGeometry(lc.width / 420, lc.height / 420), cardMaterial(texture(lc, ctx.renderer)));
    label.position.set(X(i) - 3.85 + lc.width / 840, 2.02 - lc.height / 840, -0.5);
    group.add(frame, label);
    return { frame, label };
  });

  // The data line running under all stations, with pulses travelling forward.
  const line = new Ribbon(900, 0.035);
  group.add(line.mesh);
  const ember = new THREE.Color(C.ember), emberHi = new THREE.Color(C.emberHi), ivory = new THREE.Color(C.text), grey = new THREE.Color(C.muted3), white = new THREE.Color('#ffffff');

  // Particle targets.
  const ruPts = textPoints(RU, `500 120px ${FONT.mono}`, 7.0, N, rng);
  const frPts = textPoints(FR, `500 120px ${FONT.sans}`, 7.4, N, rng);
  const cardCanvas = overlayBlock({ head: 'VOIX • RUSSE → FRANÇAIS', color: C.ember, orig: RU, line: quote(FR.replace('deux sur B.', 'deux\u00a0sur\u00a0B.')), s: 3.2 });
  const CARD_W = 5.4;
  const cardH = (CARD_W * cardCanvas.height) / cardCanvas.width;
  const seeds = new Float32Array(N * 8);
  for (let j = 0; j < N * 8; j++) seeds[j] = rng();

  const geo = new THREE.BufferGeometry();
  const pos = new Float32Array(N * 3), col = new Float32Array(N * 3), size = new Float32Array(N), alpha = new Float32Array(N);
  geo.setAttribute('position', new THREE.BufferAttribute(pos, 3));
  geo.setAttribute('aColor', new THREE.BufferAttribute(col, 3));
  geo.setAttribute('aSize', new THREE.BufferAttribute(size, 1));
  geo.setAttribute('aAlpha', new THREE.BufferAttribute(alpha, 1));
  const pmat = pointsMaterial();
  const points = new THREE.Points(geo, pmat);
  points.frustumCulled = false;
  group.add(points);

  // VAD annotations (follow the signal).
  const follow = new THREE.Group();
  group.add(follow);
  const bracketC = canvas(1500, 560);
  {
    const b = bracketC.ctx;
    b.strokeStyle = C.ember;
    b.lineWidth = 4;
    b.globalAlpha = 0.85;
    roundRect(b, 4, 4, 1492, 552, 26);
    b.setLineDash([18, 14]);
    b.stroke();
  }
  const bracket = new THREE.Mesh(new THREE.PlaneGeometry(3.9, 1.46), cardMaterial(texture(bracketC.c, ctx.renderer)));
  bracket.position.set(0.5, 0, 0.2);
  const tParole = tag('PAROLE', C.ember, ctx.renderer);
  const parole = new THREE.Mesh(new THREE.PlaneGeometry(tParole.w / 520, tParole.h / 520), cardMaterial(tParole.tex));
  parole.position.set(-1.45 + tParole.w / 1040, 1.0, 0.2);
  const tBruit = tag('BRUIT', C.muted, ctx.renderer);
  const bruit1 = new THREE.Mesh(new THREE.PlaneGeometry(tBruit.w / 520, tBruit.h / 520), cardMaterial(tBruit.tex));
  const bruit2 = bruit1.clone();
  bruit2.material = cardMaterial(tBruit.tex);
  bruit1.position.set(-2.8, 0.75, 0.2);
  bruit2.position.set(3.25, 0.75, 0.2);
  const card = new THREE.Mesh(new THREE.PlaneGeometry(CARD_W, cardH), cardMaterial(texture(cardCanvas, ctx.renderer)));
  card.position.set(0, -0.1, 0.35);
  follow.add(bracket, parole, bruit1, bruit2, card);

  // DOM: kicker, privacy line, and the app's stage strip (Accueil → bottom of the scene).
  const layer = el(ctx.dom, '', { inset: '0' }, 'layer');
  const kicker = el(layer, `LE PIPELINE <span style="color:${C.muted2}">· TOUT SE PASSE SUR VOTRE PC</span>`, { left: '150px', top: '92px', font: `500 15px ${FONT.mono}`, letterSpacing: '0.3em', color: C.ember });
  const privacy = el(layer, 'AUCUN COMPTE · AUCUNE CLÉ API · AUCUNE VOIX ENVOYÉE', { right: '150px', top: '92px', font: `500 15px ${FONT.mono}`, letterSpacing: '0.24em', color: C.muted2 });
  const strip = el(layer, '', { left: '150px', right: '150px', bottom: '74px', display: 'grid', gridTemplateColumns: 'repeat(5, 1fr)', gap: '22px' });
  const items = STATIONS.map((s) => {
    const it = el(strip, '', { position: 'relative' });
    it.style.position = 'relative';
    const bar = el(it, '', { left: '0', right: '0', top: '0', height: '3px', background: 'rgba(255,255,255,.08)', borderRadius: '2px' });
    bar.style.position = 'relative';
    const fill = el(bar, '', { left: '0', top: '0', bottom: '0', width: '0%', background: C.ok, borderRadius: '2px', boxShadow: `0 0 12px ${C.ok}` });
    const lab = el(it, s.stage[0], { font: `400 21px ${FONT.sans}`, color: C.text });
    lab.style.position = 'relative';
    lab.style.marginTop = '16px';
    const sub = el(it, s.stage[1], { font: `400 15px ${FONT.mono}`, color: C.muted2 });
    sub.style.position = 'relative';
    sub.style.marginTop = '6px';
    return { it, fill, lab, sub };
  });

  const tmp = new THREE.Color();
  return {
    name: 'technology',
    from: 12,
    to: 22,
    group,
    layer,
    bg: (t) => ({ ember: 0.7, ice: 0.35, emberPos: [0.5, 0.48], icePos: [0.9, 0.85], dust: 0.8, lift: 1 - 0.3 * smooth(range(t, 21, 22)) }),
    cam(t) {
      const x = camX(t);
      return {
        pos: new THREE.Vector3(x - 1.7, 0.5 + Math.sin(t * 0.5) * 0.08, 10.9 - Math.sin((t - 12) * 0.3) * 0.3),
        target: new THREE.Vector3(x + 0.35, 0.05, 0),
      };
    },
    update(t) {
      const sceneA = smooth(range(t, 11.0, 12.2)) * (1 - smooth(range(t, 21.6, 22.4)));
      const cx = camX(t);

      stations.forEach(({ frame, label }, i) => {
        const a = smooth(range(t, T(i) - 1.6, T(i) - 0.6)) * (1 - smooth(range(t, T(i) + 1.6, T(i) + 2.6))) * sceneA;
        frame.material.uniforms.uOpacity.value = a * 0.9;
        label.material.uniforms.uOpacity.value = a;
        label.material.uniforms.uReveal.value = 0.15 + easeInOut(range(t, T(i) - 0.8, T(i) + 0.2)) * 1.0;
      });

      line.material.uniforms.uOpacity.value = sceneA * 0.9;
      line.update((i, u, o) => {
        const x = lerp(X(0) - 8, X(4) + 8, u);
        o[0] = x; o[1] = -1.95; o[2] = -0.2;
        const pulse = Math.pow(Math.max(0, Math.sin(x * 0.55 - t * 7)), 24);
        tmp.copy(ember).lerp(white, pulse * 0.5);
        o[3] = tmp.r; o[4] = tmp.g; o[5] = tmp.b;
        const near = Math.exp(-(((x - cx) / 9) ** 2));
        o[6] = (0.18 + pulse * 0.9) * (0.3 + 0.7 * near);
      });

      // Morph weights (each particle has its own small delay).
      for (let j = 0; j < N; j++) {
        const s0 = seeds[j * 8], s1 = seeds[j * 8 + 1], s2 = seeds[j * 8 + 2], s3 = seeds[j * 8 + 3], s4 = seeds[j * 8 + 4], s5 = seeds[j * 8 + 5];
        const d = s5 * 0.4;
        const k1 = easeInOut(range(t, 10.9 + d, 12.3 + d));
        const kv = easeInOut(range(t, 14.0 + d * 0.5, 14.9 + d * 0.5));
        const k3 = easeInOut(range(t, 15.9 + d, 17.0 + d));
        const k4 = easeInOut(range(t, 17.9 + d, 19.0 + d));
        const k5 = easeInOut(range(t, 19.9 + d * 0.5, 20.6 + d * 0.5));

        // Cloud (the callouts of scene 2, dissolved).
        const th = s0 * Math.PI * 2, ph = Math.acos(2 * s1 - 1), r = 0.7 + s2 * 2.4;
        let px = cx + 0.4 + r * Math.sin(ph) * Math.cos(th) * 1.4;
        let py = r * Math.cos(ph) * 0.7;
        let pz = -1.2 + r * Math.sin(ph) * Math.sin(th) * 0.8;

        // Waveform: 7 layers; speech in the middle, gunshots on the sides.
        const layerK = j % 7;
        const wx = (s3 - 0.5) * 7.6;
        const sp = smooth(range(wx, -1.9, -1.2)) * (1 - smooth(range(wx, 2.4, 3.0)));
        const n1 = Math.exp(-(((wx + 2.75) / 0.32) ** 2)), n2 = Math.exp(-(((wx - 3.25) / 0.22) ** 2));
        const noise = Math.max(n1, n2);
        const spike = (s4 - 0.5) * 2 * Math.abs(Math.sin(wx * 37 + s0 * 6));
        let wy = sp * speech(wx, t * 1.4, layerK * 0.5) * 0.95 + noise * spike * 1.45 + (1 - sp) * (s4 - 0.5) * 0.05;
        const isNoise = noise > 0.2 && sp < 0.3;
        // VAD: what is not speech falls away and fades.
        const fall = isNoise ? kv : kv * (1 - sp) * 0.5;
        wy -= fall * (1.2 + s2 * 2.8);
        const wz = (layerK - 3) * 0.2 + (s1 - 0.5) * 0.06;
        px = lerp(px, cx + wx, k1);
        py = lerp(py, wy, k1);
        pz = lerp(pz, wz, k1);

        // Russian characters, then French, then into the overlay block.
        const sw = Math.sin(Math.PI * k3) * (s0 - 0.5) * 2.4 + Math.sin(Math.PI * k4) * (s2 - 0.5) * 2.0;
        px = lerp(px, cx + ruPts[j * 2], k3);
        py = lerp(py, ruPts[j * 2 + 1] + 0.05, k3);
        pz = lerp(pz, 0.2, k3);
        px = lerp(px, cx + frPts[j * 2], k4);
        py = lerp(py, frPts[j * 2 + 1] + 0.05, k4);
        pz = lerp(pz, 0.2, k4) + sw;
        px = lerp(px, cx + frPts[j * 2] * 0.62 - 0.25, k5);
        py = lerp(py, frPts[j * 2 + 1] * 0.62 - 0.1 - cardH * 0.22, k5);
        pz = lerp(pz, 0.36, k5);
        pos[j * 3] = px; pos[j * 3 + 1] = py; pos[j * 3 + 2] = pz;

        // Colour and light.
        tmp.copy(ivory).multiplyScalar(0.7);
        const wc = isNoise ? grey : ember.clone().lerp(emberHi, s3);
        tmp.lerp(wc, k1);
        if (isNoise) tmp.lerp(grey, kv);
        tmp.lerp(ivory.clone().multiplyScalar(0.85), k3);
        tmp.lerp(emberHi.clone().lerp(white, 0.15 + s1 * 0.3), k4);
        col[j * 3] = tmp.r; col[j * 3 + 1] = tmp.g; col[j * 3 + 2] = tmp.b;
        let a = lerp(0.16, isNoise ? 0.75 : 0.85, k1);
        a *= 1 - fall * 0.97 * (1 - k3);
        a = lerp(a, 0.62, k3);
        a *= 1 - smooth(range(t, 20.35, 20.95));
        alpha[j] = a;
        size[j] = lerp(0.03, 0.017, k1) * lerp(1, 0.78, k3) * lerp(1, 0.8, k5);
      }
      geo.attributes.position.needsUpdate = true;
      geo.attributes.aColor.needsUpdate = true;
      geo.attributes.aSize.needsUpdate = true;
      geo.attributes.aAlpha.needsUpdate = true;
      pmat.uniforms.uOpacity.value = sceneA;

      follow.position.x = cx;
      const vad = smooth(range(t, 14.4, 15.0)) * (1 - smooth(range(t, 15.9, 16.4)));
      bracket.material.uniforms.uOpacity.value = vad * sceneA;
      parole.material.uniforms.uOpacity.value = vad * sceneA;
      bruit1.material.uniforms.uOpacity.value = vad * sceneA * 0.9;
      bruit2.material.uniforms.uOpacity.value = vad * sceneA * 0.9;
      card.material.uniforms.uOpacity.value = smooth(range(t, 20.3, 20.9)) * (1 - smooth(range(t, 21.7, 22.3)));
      card.scale.setScalar(lerp(0.96, 1, easeInOut(range(t, 20.3, 21.2))));

      const ui = smooth(range(t, 12.0, 12.8)) * (1 - smooth(range(t, 21.2, 21.8)));
      show(kicker, ui);
      show(privacy, ui);
      show(strip, ui, { y: (1 - smooth(range(t, 12.0, 12.8))) * 16 });
      items.forEach(({ fill, lab, sub }, i) => {
        const p = range(t, T(i) - 0.9, T(i) + 1.0);
        fill.style.width = `${(p * 100).toFixed(1)}%`;
        const active = t >= T(i) - 0.9;
        lab.style.color = active ? C.textHi : C.muted2;
        sub.style.color = active ? C.text3 : C.muted4;
      });
    },
  };
}
