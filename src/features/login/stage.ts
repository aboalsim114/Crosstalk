// The login page's 3D stage (two interlocked rings, particles, bloom),
// ported from the Claude Design file "Crosstalk Login".

import * as T from 'three';
import { EffectComposer } from 'three/addons/postprocessing/EffectComposer.js';
import { RenderPass } from 'three/addons/postprocessing/RenderPass.js';
import { UnrealBloomPass } from 'three/addons/postprocessing/UnrealBloomPass.js';
import { OutputPass } from 'three/addons/postprocessing/OutputPass.js';

export type LoginPhase = 'idle' | 'loading' | 'success';

export interface LoginStageState {
  phase: LoginPhase;
  focus: boolean;
  reducedMotion: boolean;
  /** Pointer position over the window, -1..1. */
  mx: number;
  my: number;
  introStart: number;
}

/** One-shot effects the page triggers: error shake, success flare, shock wave. */
export interface LoginFx {
  err: number;
  flare: number;
  wave: number;
}

export interface LoginStage {
  dispose(): void;
}

export function createLoginStage(el: HTMLElement, get: () => LoginStageState, fx: LoginFx): LoginStage | null {
  let R: T.WebGLRenderer;
  try {
    R = new T.WebGLRenderer({ antialias: true, powerPreference: 'high-performance' });
  } catch {
    return null;
  }
  // The design is drawn for 1000×640; the stage fills the window and follows its size.
  let W = el.clientWidth || 1000,
    H = el.clientHeight || 640;
  R.setPixelRatio(Math.min(2, devicePixelRatio || 1));
  R.setSize(W, H);
  R.setClearColor(0x08090c, 1);
  R.toneMapping = T.ACESFilmicToneMapping;
  R.toneMappingExposure = 1.0;
  R.outputColorSpace = T.SRGBColorSpace;
  R.domElement.style.cssText = 'position:absolute;inset:0;width:100%;height:100%;display:block';
  el.appendChild(R.domElement);
  const scene = new T.Scene();
  scene.fog = new T.FogExp2(0x08090c, 0.05);
  const cam = new T.PerspectiveCamera(30, W / H, 0.1, 100);
  cam.position.set(0, 0.3, 8.4);

  const pm = new T.PMREMGenerator(R),
    es = new T.Scene();
  es.add(new T.Mesh(new T.BoxGeometry(24, 24, 24), new T.MeshBasicMaterial({ color: 0x030304, side: T.BackSide })));
  const panel = (c: number, i: number, x: number, y: number, z: number, w: number, h: number) => {
    const m = new T.Mesh(new T.PlaneGeometry(w, h), new T.MeshBasicMaterial({ color: new T.Color(c).multiplyScalar(i), side: T.DoubleSide }));
    m.position.set(x, y, z);
    m.lookAt(0, 0, 0);
    es.add(m);
  };
  panel(0xffffff, 1.4, 0, 9, 2, 8, 2.5);
  panel(0xff643b, 1.6, -8, 1, -3, 2.5, 8);
  panel(0x91dceb, 1.1, 8, 0.5, -2, 2, 7);
  scene.environment = pm.fromScene(es, 0.04).texture;

  const shell = new T.MeshPhysicalMaterial({ color: 0x121317, roughness: 0.32, metalness: 0.9, clearcoat: 0.8, clearcoatRoughness: 0.25, envMapIntensity: 0.7 });
  const chrome = new T.MeshStandardMaterial({ color: 0x8a8f97, roughness: 0.22, metalness: 1, envMapIntensity: 0.6 });
  const emA = new T.MeshStandardMaterial({ color: 0xff643b, emissive: 0xff643b, emissiveIntensity: 2.2, toneMapped: false });
  const emB = new T.MeshStandardMaterial({ color: 0x91dceb, emissive: 0x91dceb, emissiveIntensity: 1.6, toneMapped: false });
  const mkRing = (em: T.MeshStandardMaterial) => {
    const g = new T.Group();
    g.add(new T.Mesh(new T.TorusGeometry(1, 0.13, 48, 220), shell));
    for (const s of [-1, 1]) {
      const lip = new T.Mesh(new T.TorusGeometry(1, 0.018, 12, 220), chrome);
      lip.position.z = s * 0.118;
      g.add(lip);
    }
    g.add(new T.Mesh(new T.TorusGeometry(1.128, 0.011, 10, 220), em));
    g.add(new T.Mesh(new T.TorusGeometry(0.872, 0.008, 10, 220), em));
    for (let i = 0; i < 24; i++) {
      const a = (i / 24) * Math.PI * 2,
        t = new T.Mesh(new T.BoxGeometry(0.012, 0.05, 0.02), chrome);
      t.position.set(Math.cos(a) * 1.17, Math.sin(a) * 1.17, 0);
      t.rotation.z = a + Math.PI / 2;
      t.scale.y = i % 6 === 0 ? 2 : 1;
      g.add(t);
    }
    return { g, base: em.emissiveIntensity };
  };
  const rig = new T.Group();
  scene.add(rig);
  const A = mkRing(emA),
    B = mkRing(emB);
  rig.add(A.g, B.g);
  const core = new T.Mesh(new T.SphereGeometry(0.07, 24, 16), new T.MeshBasicMaterial({ color: 0xffe6da, toneMapped: false }));
  rig.add(core);

  scene.add(new T.AmbientLight(0x1c1e24, 0.4));
  const key = new T.SpotLight(0xfff1e6, 90, 24, Math.PI / 6, 0.7, 1.2);
  key.position.set(0, 7, 3.5);
  scene.add(key, key.target);
  const rimA = new T.PointLight(0xff643b, 30, 14, 1.5);
  rimA.position.set(-3.6, 1, -2.4);
  scene.add(rimA);
  const rimB = new T.PointLight(0x91dceb, 22, 14, 1.5);
  rimB.position.set(1.4, -0.2, -2.4);
  scene.add(rimB);
  const coreL = new T.PointLight(0xff8a5c, 0, 6, 2);
  scene.add(coreL);

  const ctex = (sz: number, stops: [number, string][]) => {
    const c = document.createElement('canvas');
    c.width = c.height = sz;
    const g = c.getContext('2d')!;
    const gr = g.createRadialGradient(sz / 2, sz / 2, 0, sz / 2, sz / 2, sz / 2);
    stops.forEach((s) => gr.addColorStop(s[0], s[1]));
    g.fillStyle = gr;
    g.fillRect(0, 0, sz, sz);
    const t = new T.CanvasTexture(c);
    t.colorSpace = T.SRGBColorSpace;
    return t;
  };
  const glow = new T.Sprite(
    new T.SpriteMaterial({
      map: ctex(256, [
        [0, 'rgba(255,100,59,1)'],
        [0.35, 'rgba(255,100,59,.3)'],
        [1, 'rgba(255,100,59,0)'],
      ]),
      blending: T.AdditiveBlending,
      depthWrite: false,
      transparent: true,
      opacity: 0.3,
    }),
  );
  glow.scale.set(7, 7, 1);
  scene.add(glow);
  scene.background = new T.Color(0x040405);
  const wave = new T.Mesh(
    new T.TorusGeometry(1, 0.006, 8, 240),
    new T.MeshBasicMaterial({ color: 0xffd2c2, transparent: true, opacity: 0, blending: T.AdditiveBlending, depthWrite: false, toneMapped: false }),
  );
  scene.add(wave);

  const N = 1400,
    pos = new Float32Array(N * 3),
    col = new Float32Array(N * 3),
    Bp = new Float32Array(N * 3),
    P = new Float32Array(N * 2);
  const cE = new T.Color(0xff643b),
    cI = new T.Color(0x91dceb),
    cW = new T.Color(0xf2f1ee);
  for (let i = 0; i < N; i++) {
    const r = 2.2 + Math.random() * 4,
      a = Math.random() * 6.283,
      b = Math.acos(2 * Math.random() - 1);
    Bp[i * 3] = r * Math.sin(b) * Math.cos(a);
    Bp[i * 3 + 1] = r * Math.cos(b) * 0.6;
    Bp[i * 3 + 2] = r * Math.sin(b) * Math.sin(a) * 0.7 - 1;
    P[i * 2] = Math.random();
    P[i * 2 + 1] = Math.random() < 0.5 ? 0 : 1;
    const c = Math.random() < 0.12 ? cW : P[i * 2 + 1] ? cI : cE;
    col[i * 3] = c.r;
    col[i * 3 + 1] = c.g;
    col[i * 3 + 2] = c.b;
  }
  const pg = new T.BufferGeometry();
  pg.setAttribute('position', new T.BufferAttribute(pos, 3));
  pg.setAttribute('color', new T.BufferAttribute(col, 3));
  const pmat = new T.PointsMaterial({
    size: 0.03,
    map: ctex(64, [
      [0, 'rgba(255,255,255,1)'],
      [0.4, 'rgba(255,255,255,.5)'],
      [1, 'rgba(255,255,255,0)'],
    ]),
    vertexColors: true,
    transparent: true,
    opacity: 0,
    blending: T.AdditiveBlending,
    depthWrite: false,
  });
  scene.add(new T.Points(pg, pmat));

  let comp: EffectComposer | null = null,
    bloom: UnrealBloomPass | null = null;
  try {
    comp = new EffectComposer(R);
    comp.setPixelRatio(Math.min(2, devicePixelRatio || 1));
    comp.setSize(W, H);
    comp.addPass(new RenderPass(scene, cam));
    bloom = new UnrealBloomPass(new T.Vector2(W, H), 0.5, 0.5, 0.82);
    comp.addPass(bloom);
    comp.addPass(new OutputPass());
  } catch {
    comp = null;
  }

  const CX = -1.25,
    CY = 0.62;
  const ang: Record<'ax' | 'ay' | 'bx' | 'bz' | 'by', number> = { ax: 0.9, ay: 0.4, bx: -0.5, bz: 0.8, by: 1.2 };
  const lv = { lock: 0, spin: 1, glow: 1, conv: 0, pOp: 0, sep: 0 };
  const wrap = (a: number) => a - Math.PI * 2 * Math.round(a / (Math.PI * 2));
  const sm = (a: number, b: number, x: number) => {
    const t = Math.min(1, Math.max(0, (x - a) / (b - a)));
    return t * t * (3 - 2 * t);
  };
  const v = new T.Vector3(),
    cR = new T.Color(0xff3a2a),
    cBb = new T.Color(0x91dceb);
  const resize = () => {
    const w = el.clientWidth,
      h = el.clientHeight;
    if (!w || !h || (w === W && h === H)) return;
    W = w;
    H = h;
    R.setSize(W, H);
    comp?.setSize(W, H);
    cam.aspect = W / H;
    cam.updateProjectionMatrix();
  };
  const ro = typeof ResizeObserver === 'undefined' ? null : new ResizeObserver(resize);
  ro?.observe(el);
  let last = performance.now(),
    tt = 0,
    raf = 0,
    dead = false;
  const loop = (now: number) => {
    if (dead) return;
    raf = requestAnimationFrame(loop);
    const dt = Math.min(0.05, (now - last) / 1000);
    last = now;
    const st = get(),
      rm = st.reducedMotion,
      ph = st.phase;
    tt += dt;
    const ie = (now - st.introStart) / 1000,
      kIn = rm ? 1 : sm(0.3, 2.6, ie);
    const tg =
      ph === 'success'
        ? { lock: 1, spin: 0, glow: 2, conv: 0, pOp: 0.5, sep: 0 }
        : ph === 'loading'
          ? { lock: 0, spin: 5, glow: 1.9, conv: 1, pOp: 1, sep: -0.12 }
          : { lock: 0, spin: st.focus ? 1.7 : 1, glow: st.focus ? 1.25 : 1, conv: 0, pOp: 0.75, sep: 0 };
    const f = 1 - Math.exp(-dt * (ph === 'success' ? 3.2 : 2.6));
    for (const k of Object.keys(tg) as (keyof typeof tg)[]) lv[k] += (tg[k] - lv[k]) * f;
    const sp = (rm ? 0.15 : 1) * lv.spin * dt;
    if (ph === 'success') {
      for (const k of Object.keys(ang) as (keyof typeof ang)[]) {
        ang[k] = wrap(ang[k]);
        ang[k] += (0 - ang[k]) * (1 - Math.exp(-dt * 3.4));
      }
    } else {
      ang.ax += sp * 0.5;
      ang.ay += sp * 0.33;
      ang.bx -= sp * 0.41;
      ang.bz += sp * 0.27;
      ang.by += sp * 0.22;
    }
    fx.err = Math.max(0, fx.err - dt * 1.6);
    fx.flare = Math.max(0, fx.flare - dt * 0.9);
    const err = fx.err,
      shake = rm ? 0 : Math.sin(tt * 70) * 0.1 * err * err;
    const L = lv.lock,
      sep = 0.3 + lv.sep;
    const bob = rm ? 0 : Math.sin(tt * 0.8) * 0.06 * (1 - L);
    const intro = 1 - kIn;
    rig.position.set(CX + shake, CY + bob - intro * 0.5, -intro * 2.5);
    rig.rotation.set(st.my * 0.08 * (1 - L * 0.6), st.mx * 0.14 * (1 - L * 0.6) - intro * 1.2, 0);
    A.g.position.set(-sep * (1 - L) - 0.58 * L, 0, 0);
    B.g.position.set(sep * (1 - L) + 0.58 * L, 0, 0);
    A.g.rotation.set(ang.ax, ang.ay, 0);
    B.g.rotation.set(ang.bx, ang.by, ang.bz);
    rig.scale.setScalar(0.78 + L * 0.04 + fx.flare * 0.05);
    emA.emissiveIntensity = A.base * (0.5 + lv.glow * 0.5) * kIn * (1 + err * 0.8) + fx.flare * 3;
    emB.emissive.copy(cBb).lerp(cR, err);
    emB.color.copy(emB.emissive);
    emB.emissiveIntensity = B.base * (0.5 + lv.glow * 0.5) * kIn + fx.flare * 2.5;
    core.scale.setScalar(0.2 + lv.conv * 0.9 + L * 1.4 + fx.flare * 3);
    coreL.position.copy(rig.position);
    coreL.intensity = (lv.conv * 6 + L * 8 + fx.flare * 30) * kIn;
    key.intensity = 90 * kIn;
    key.target.position.copy(rig.position);
    key.position.x = CX;
    rimA.intensity = 30 * kIn * (1 + err);
    rimB.intensity = 22 * kIn;
    glow.position.set(rig.position.x, rig.position.y, -1.6);
    glow.material.opacity = (0.07 + lv.glow * 0.05 + fx.flare * 0.4) * kIn;
    if (fx.wave >= 0) {
      fx.wave += dt;
      const w = fx.wave;
      wave.position.copy(rig.position);
      wave.scale.setScalar(0.8 + w * 4.5);
      wave.material.opacity = Math.max(0, 0.9 - w * 0.75);
      if (w > 1.3) fx.wave = -1;
    } else wave.material.opacity = 0;
    rig.updateMatrixWorld(true);
    const conv = lv.conv;
    for (let i = 0; i < N; i++) {
      const p = P[i * 2],
        side = P[i * 2 + 1];
      let x = Bp[i * 3] + CX * 0.4 + Math.sin(tt * 0.2 + p * 6.28) * 0.2,
        y = Bp[i * 3 + 1] + CY * 0.5 + Math.cos(tt * 0.17 + p * 9) * 0.15,
        z = Bp[i * 3 + 2];
      if (conv > 0.003) {
        const a = p * 6.283 + tt * (side ? -1.6 : 1.6);
        v.set(Math.cos(a), Math.sin(a), 0).applyMatrix4((side ? B : A).g.matrixWorld);
        x += (v.x - x) * conv;
        y += (v.y - y) * conv;
        z += (v.z - z) * conv;
      }
      pos[i * 3] = x;
      pos[i * 3 + 1] = y;
      pos[i * 3 + 2] = z;
    }
    pg.attributes.position.needsUpdate = true;
    pmat.opacity = lv.pOp * kIn * 0.85;
    pmat.size = 0.028 + conv * 0.012;
    const orb = rm ? 0 : Math.sin(tt * 0.09) * 0.07 + st.mx * 0.05;
    cam.position.set(Math.sin(orb) * 8.4, 0.3 - st.my * 0.1, Math.cos(orb) * 8.4);
    cam.lookAt(-0.3, 0.1, 0);
    if (comp && bloom) {
      bloom.strength = (0.35 + lv.glow * 0.15 + fx.flare * 1.1) * kIn;
      comp.render();
    } else R.render(scene, cam);
  };
  raf = requestAnimationFrame(loop);

  return {
    dispose() {
      dead = true;
      ro?.disconnect();
      cancelAnimationFrame(raf);
      comp?.dispose();
      pm.dispose();
      R.dispose();
      R.domElement.remove();
    },
  };
}
