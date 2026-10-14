// Crosstalk showcase: a deterministic film. `window.__seek(t)` renders the frame at
// time t (seconds); scripts/render.mjs calls it for every frame and pipes the
// screenshots to FFmpeg. Open the page with ?play to watch it live, ?t=12.5 to freeze.
import '@fontsource/instrument-serif/400.css';
import '@fontsource/instrument-serif/400-italic.css';
import '@fontsource/geist-sans/300.css';
import '@fontsource/geist-sans/400.css';
import '@fontsource/geist-sans/500.css';
import '@fontsource/geist-sans/600.css';
import '@fontsource/jetbrains-mono/400.css';
import '@fontsource/jetbrains-mono/500.css';
import '@fontsource/jetbrains-mono/600.css';
import * as THREE from 'three';
import { EffectComposer } from 'three/addons/postprocessing/EffectComposer.js';
import { RenderPass } from 'three/addons/postprocessing/RenderPass.js';
import { UnrealBloomPass } from 'three/addons/postprocessing/UnrealBloomPass.js';
import { OutputPass } from 'three/addons/postprocessing/OutputPass.js';
import { backdropMaterial, pointsMaterial } from './materials.js';
import { lerp, mulberry32, range, smooth, easeInOut } from './util.js';
import intro from './scenes/s1-intro.js';
import problem from './scenes/s2-problem.js';
import technology from './scenes/s3-technology.js';
import application from './scenes/s4-application.js';
import experience from './scenes/s5-experience.js';
import finale from './scenes/s6-finale.js';

export const W = 1920, H = 1080, FPS = 30, DURATION = 50;

const canvasEl = document.getElementById('gl');
const renderer = new THREE.WebGLRenderer({ canvas: canvasEl, antialias: true, preserveDrawingBuffer: true, powerPreference: 'high-performance' });
renderer.setPixelRatio(1);
renderer.setSize(W, H, false);
renderer.setClearColor(0x020203, 1);

const scene = new THREE.Scene();
const camera = new THREE.PerspectiveCamera(35, W / H, 0.05, 400);
scene.add(camera);

// Backdrop glued to the camera, far away.
const bgMat = backdropMaterial();
const bgDist = 300;
const bgH = 2 * bgDist * Math.tan(THREE.MathUtils.degToRad(40) / 2) * 1.15;
const backdrop = new THREE.Mesh(new THREE.PlaneGeometry(bgH * (W / H), bgH), bgMat);
backdrop.position.z = -bgDist;
backdrop.renderOrder = -10;
camera.add(backdrop);

// Dust in the light: wraps around the camera so it is everywhere.
const DUST = 1400;
const dustGeo = new THREE.BufferGeometry();
{
  const r = mulberry32(7);
  const p = new Float32Array(DUST * 3), c = new Float32Array(DUST * 3), s = new Float32Array(DUST), a = new Float32Array(DUST);
  const warm = new THREE.Color('#ffc68a'), cold = new THREE.Color('#ece6dc');
  for (let i = 0; i < DUST; i++) {
    p[i * 3] = (r() - 0.5) * 40; p[i * 3 + 1] = (r() - 0.5) * 22; p[i * 3 + 2] = (r() - 0.5) * 40;
    const col = r() < 0.35 ? warm : cold;
    c[i * 3] = col.r; c[i * 3 + 1] = col.g; c[i * 3 + 2] = col.b;
    s[i] = 0.012 + r() * r() * 0.05;
    a[i] = 0.15 + r() * 0.45;
  }
  dustGeo.setAttribute('position', new THREE.BufferAttribute(p, 3));
  dustGeo.setAttribute('aColor', new THREE.BufferAttribute(c, 3));
  dustGeo.setAttribute('aSize', new THREE.BufferAttribute(s, 1));
  dustGeo.setAttribute('aAlpha', new THREE.BufferAttribute(a, 1));
}
const dustMat = pointsMaterial();
dustMat.uniforms.uWrap.value.set(40, 22, 40);
dustMat.uniforms.uDof.value = 1.2;
const dust = new THREE.Points(dustGeo, dustMat);
dust.frustumCulled = false;
scene.add(dust);

const composer = new EffectComposer(renderer);
composer.setPixelRatio(1);
composer.setSize(W, H);
composer.addPass(new RenderPass(scene, camera));
const bloom = new UnrealBloomPass(new THREE.Vector2(W / 2, H / 2), 0.6, 0.65, 0.1);
composer.addPass(bloom);
composer.addPass(new OutputPass());

// Film grain: a few fixed noise frames, picked by frame number (deterministic).
const grain = document.getElementById('grain');
const gctx = grain.getContext('2d');
const grainFrames = [];
{
  const r = mulberry32(99);
  for (let f = 0; f < 8; f++) {
    const img = gctx.createImageData(grain.width, grain.height);
    for (let i = 0; i < img.data.length; i += 4) {
      const v = 128 + (r() - 0.5) * 255;
      img.data[i] = img.data[i + 1] = img.data[i + 2] = v;
      img.data[i + 3] = 255;
    }
    grainFrames.push(img);
  }
}
const fade = document.getElementById('fade');

async function loadFonts() {
  const specs = [
    '400 40px "Instrument Serif"', 'italic 400 40px "Instrument Serif"',
    '300 40px "Geist Sans"', '400 40px "Geist Sans"', '500 40px "Geist Sans"', '600 40px "Geist Sans"',
    '400 40px "JetBrains Mono"', '500 40px "JetBrains Mono"', '600 40px "JetBrains Mono"',
  ];
  await Promise.all(specs.map((s) => document.fonts.load(s, 'AaÉé«»→·')));
  await Promise.all(['400', '500', '600'].map((w) => document.fonts.load(`${w} 40px "JetBrains Mono"`, 'Двое идут на B')));
  await document.fonts.ready;
}

async function loadScreens() {
  const loader = new THREE.TextureLoader();
  const names = ['02-dashboard', '04-audio', '03-overlay', '05-languages'];
  const out = {};
  await Promise.all(
    names.map(async (n) => {
      const t = await loader.loadAsync(`./screens/${n}.jpg`);
      t.colorSpace = THREE.SRGBColorSpace;
      t.anisotropy = renderer.capabilities.getMaxAnisotropy();
      t.minFilter = THREE.LinearMipmapLinearFilter;
      out[n] = t;
    }),
  );
  return out;
}

const tmpA = new THREE.Vector3(), tmpB = new THREE.Vector3();
let scenes = [];

function pose(sc, t) {
  const p = sc.cam(t);
  return { pos: p.pos, target: p.target, fov: p.fov ?? 35, bg: sc.bg ? sc.bg(t) : {} };
}

function blendBg(a, b, k) {
  const keys = new Set([...Object.keys(a), ...Object.keys(b)]);
  const o = {};
  for (const key of keys) {
    const va = a[key] ?? b[key], vb = b[key] ?? a[key];
    o[key] = Array.isArray(va) ? va.map((x, i) => lerp(x, vb[i], k)) : lerp(va, vb, k);
  }
  return o;
}

function render(t) {
  // Active scene and camera blend around each cut.
  let i = scenes.findIndex((s) => t >= s.from && t < s.to);
  if (i < 0) i = t < scenes[0].from ? 0 : scenes.length - 1;
  const cur = scenes[i];
  let p = pose(cur, t);
  const next = scenes[i + 1], prev = scenes[i - 1];
  const H_NEXT = next?.blendIn ?? 0.8;
  const H_CUR = cur.blendIn ?? 0.8;
  if (next && t > cur.to - H_NEXT) {
    const k = easeInOut(range(t, cur.to - H_NEXT, cur.to + H_NEXT));
    const q = pose(next, t);
    p = { pos: tmpA.copy(p.pos).lerp(q.pos, k).clone(), target: tmpB.copy(p.target).lerp(q.target, k).clone(), fov: lerp(p.fov, q.fov, k), bg: blendBg(p.bg, q.bg, k) };
  } else if (prev && t < cur.from + H_CUR) {
    const k = easeInOut(range(t, cur.from - H_CUR, cur.from + H_CUR));
    const q = pose(prev, t);
    p = { pos: tmpA.copy(q.pos).lerp(p.pos, k).clone(), target: tmpB.copy(q.target).lerp(p.target, k).clone(), fov: lerp(q.fov, p.fov, k), bg: blendBg(q.bg, p.bg, k) };
  }
  camera.position.copy(p.pos);
  camera.fov = p.fov;
  camera.updateProjectionMatrix();
  camera.lookAt(p.target);
  camera.updateMatrixWorld(true);

  const bg = p.bg;
  bgMat.uniforms.uEmber.value = bg.ember ?? 0.3;
  bgMat.uniforms.uIce.value = bg.ice ?? 0.15;
  if (bg.emberPos) bgMat.uniforms.uEmberPos.value.set(...bg.emberPos);
  if (bg.icePos) bgMat.uniforms.uIcePos.value.set(...bg.icePos);
  bgMat.uniforms.uLift.value = bg.lift ?? 1;
  bloom.strength = bg.bloom ?? 0.6;
  bloom.threshold = bg.bloomThr ?? 0.1;

  dustMat.uniforms.uCam.value.copy(camera.position);
  dustMat.uniforms.uFocus.value = 7;
  dustMat.uniforms.uOpacity.value = (bg.dust ?? 1) * smooth(range(t, 0.6, 2.5));
  dust.position.set(Math.sin(t * 0.05) * 0.6, t * 0.06, t * 0.12);

  for (const s of scenes) {
    const on = t >= s.from - (s.pre ?? 1.5) && t <= s.to + (s.post ?? 1.5);
    s.group.visible = on;
    if (s.layer) s.layer.style.display = on ? 'block' : 'none';
    if (on) s.update(t);
  }

  composer.render();

  const f = Math.round(t * FPS);
  gctx.putImageData(grainFrames[f % grainFrames.length], 0, 0);
  const black = 1 - smooth(range(t, 0, 0.5)) * (1 - smooth(range(t, DURATION - 1.1, DURATION - 0.05)));
  fade.style.opacity = black.toFixed(3);
}

async function boot() {
  await loadFonts();
  const screens = await loadScreens();
  const ctx = { THREE, scene, camera, renderer, dom: document.getElementById('dom'), screens, W, H };
  scenes = [intro, problem, technology, application, experience, finale].map((f) => f(ctx));
  for (const s of scenes) scene.add(s.group);
  // Upload every texture once so the first frames don't stutter.
  renderer.compile(scene, camera);
  render(0);
  window.__meta = { W, H, FPS, DURATION };
  window.__hideDust = () => (dust.visible = false);
  window.__seek = (t) =>
    new Promise((res) => {
      render(t);
      requestAnimationFrame(() => res(true));
    });
  const q = new URLSearchParams(location.search);
  if (q.has('t')) render(parseFloat(q.get('t')));
  if (q.has('play')) {
    const t0 = performance.now() - (parseFloat(q.get('play')) || 0) * 1000;
    const loop = () => {
      render(((performance.now() - t0) / 1000) % DURATION);
      requestAnimationFrame(loop);
    };
    loop();
  }
  window.__ready = true;
}

boot().catch((e) => {
  console.error(e);
  window.__error = String(e?.stack || e);
});
