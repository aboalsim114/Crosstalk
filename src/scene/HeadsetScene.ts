// The cinematic 3D stage: a stylised gaming headset with LED rings, orbiting
// original → translation word pairs, an audio-reactive wave ring and dust.
// Ported from the Claude Design prototype (initThree), driven by the app store.

import * as THREE from 'three';
import { ORBIT_PAIRS } from '../state/data';
import { isLive, sceneFx, useApp, type AppState } from '../state/store';

interface Target {
  x: number;
  y: number;
  s: number;
  dim: number;
}

/** Camera framing: centred during the intro, hero on Accueil, tucked in a corner elsewhere. */
export function sceneTarget(s: Pick<AppState, 'entered' | 'page'>): Target {
  if (!s.entered) return { x: 0, y: 0.5, s: 0.82, dim: 1 };
  if (s.page === 'dash') return { x: -0.55, y: 0.82, s: 0.6, dim: 1 };
  return { x: 3.6, y: -1.85, s: 0.32, dim: 0.5 };
}

interface Burst {
  m: THREE.Mesh<THREE.TorusGeometry, THREE.MeshBasicMaterial>;
  age: number;
  sd: number;
  op: number;
  life: number;
}

export class HeadsetScene {
  private renderer: THREE.WebGLRenderer;
  private disposed = false;
  private raf = 0;
  private ro: ResizeObserver;

  private constructor(private el: HTMLElement) {
    const R = new THREE.WebGLRenderer({ antialias: true, alpha: true });
    R.setPixelRatio(Math.min(2, window.devicePixelRatio || 1));
    R.toneMapping = THREE.ACESFilmicToneMapping;
    R.toneMappingExposure = 1.15;
    R.domElement.style.cssText = 'position:absolute;inset:0;width:100%;height:100%;display:block';
    el.appendChild(R.domElement);
    this.renderer = R;
    this.ro = new ResizeObserver(() => this.resize());
  }

  private camera = new THREE.PerspectiveCamera(32, 1, 0.1, 100);

  /** Returns null when WebGL is unavailable (the UI stays fully usable without the stage). */
  static async mount(el: HTMLElement): Promise<HeadsetScene | null> {
    let scene: HeadsetScene;
    try {
      scene = new HeadsetScene(el);
    } catch (e) {
      console.warn('WebGL unavailable, 3D stage disabled', e);
      return null;
    }
    // Word sprites are drawn on canvases: make sure the faces are loaded first.
    try {
      await Promise.all([document.fonts.load('500 56px "JetBrains Mono"', 'Двое на B'), document.fonts.load('italic 76px "Instrument Serif"', 'Deux sur B')]);
    } catch {
      /* fall back to system fonts */
    }
    if (scene.disposed) return scene;
    scene.build();
    return scene;
  }

  private resize() {
    const w = this.el.clientWidth;
    const h = this.el.clientHeight;
    if (!w || !h) return;
    this.renderer.setSize(w, h, false);
    this.camera.aspect = w / h;
    this.camera.updateProjectionMatrix();
  }

  private build() {
    const T = THREE;
    const S = new T.Scene();
    const C = this.camera;
    C.position.set(0, 0, 9);
    const root = new T.Group();
    S.add(root);
    const hs = new T.Group();
    root.add(hs);

    const metal = new T.MeshPhysicalMaterial({ color: 0x1a171c, metalness: 0.88, roughness: 0.28, clearcoat: 1, clearcoatRoughness: 0.14 });
    const soft = new T.MeshStandardMaterial({ color: 0x0e0d10, roughness: 0.9, metalness: 0.05 });
    const led = new T.MeshStandardMaterial({ color: 0x120a05, emissive: 0xff8a3d, emissiveIntensity: 0.6 });
    const grill = new T.MeshBasicMaterial({ color: 0xff9a4a, wireframe: true, transparent: true, opacity: 0.25, blending: T.AdditiveBlending, depthWrite: false });

    const band = new T.Mesh(new T.TorusGeometry(1.12, 0.055, 20, 120, Math.PI), metal);
    band.position.y = -0.05;
    hs.add(band);
    const pad = new T.Mesh(new T.TorusGeometry(1.03, 0.075, 20, 96, Math.PI * 0.62), soft);
    pad.rotation.z = Math.PI * 0.19;
    pad.position.y = -0.05;
    hs.add(pad);
    const ledBand = new T.Mesh(new T.TorusGeometry(1.175, 0.008, 8, 120, Math.PI * 0.5), led);
    ledBand.rotation.z = Math.PI * 0.25;
    ledBand.position.y = -0.05;
    hs.add(ledBand);

    const cups: { g: THREE.Group; sd: number }[] = [];
    for (const sd of [-1, 1]) {
      const g = new T.Group();
      g.position.set(sd * 1.12, -0.66, 0);
      hs.add(g);
      const slider = new T.Mesh(new T.CylinderGeometry(0.04, 0.04, 0.34, 16), metal);
      slider.position.set(sd * 1.12, -0.2, 0);
      hs.add(slider);
      const fork = new T.Mesh(new T.TorusGeometry(0.56, 0.03, 12, 64, Math.PI), metal);
      fork.rotation.y = Math.PI / 2;
      fork.position.x = sd * 0.04;
      g.add(fork);
      const shell = new T.Mesh(new T.CylinderGeometry(0.5, 0.46, 0.26, 64), metal);
      shell.rotation.z = Math.PI / 2;
      shell.position.x = sd * 0.06;
      g.add(shell);
      const cush = new T.Mesh(new T.TorusGeometry(0.38, 0.13, 20, 64), soft);
      cush.rotation.y = Math.PI / 2;
      cush.position.x = -sd * 0.1;
      g.add(cush);
      const plate = new T.Mesh(new T.CylinderGeometry(0.34, 0.34, 0.03, 64), metal);
      plate.rotation.z = Math.PI / 2;
      plate.position.x = sd * 0.205;
      g.add(plate);
      const mesh = new T.Mesh(new T.CircleGeometry(0.3, 24), grill);
      mesh.rotation.y = (sd * Math.PI) / 2;
      mesh.position.x = sd * 0.225;
      g.add(mesh);
      const ring = new T.Mesh(new T.TorusGeometry(0.42, 0.016, 8, 96), led);
      ring.rotation.y = Math.PI / 2;
      ring.position.x = sd * 0.2;
      g.add(ring);
      cups.push({ g, sd });
    }

    const mkWord = (txt: string, font: string, color: string, h: number) => {
      const cv = document.createElement('canvas');
      const c0 = cv.getContext('2d')!;
      c0.font = font;
      const w = Math.ceil(c0.measureText(txt).width) + 60;
      cv.width = w;
      cv.height = 120;
      const c = cv.getContext('2d')!;
      c.font = font;
      c.fillStyle = color;
      c.textBaseline = 'middle';
      c.shadowColor = color;
      c.shadowBlur = 22;
      c.fillText(txt, 30, 62);
      const tx = new T.CanvasTexture(cv);
      tx.colorSpace = T.SRGBColorSpace;
      const sp = new T.Sprite(new T.SpriteMaterial({ map: tx, transparent: true, depthWrite: false, opacity: 0 }));
      sp.scale.set((w / 120) * h, h, 1);
      root.add(sp);
      return sp;
    };
    const words = ORBIT_PAIRS.map(([a, b], i) => ({
      a: mkWord(a, '500 56px "JetBrains Mono", monospace', '#9fdcff', 0.3),
      b: mkWord(b, 'italic 76px "Instrument Serif", serif', '#ffc68a', 0.36),
      ph: i / ORBIT_PAIRS.length,
    }));

    const gc = document.createElement('canvas');
    gc.width = gc.height = 256;
    const g2 = gc.getContext('2d')!;
    const gr = g2.createRadialGradient(128, 128, 0, 128, 128, 128);
    gr.addColorStop(0, 'rgba(255,170,90,.55)');
    gr.addColorStop(0.3, 'rgba(255,120,50,.18)');
    gr.addColorStop(1, 'rgba(0,0,0,0)');
    g2.fillStyle = gr;
    g2.fillRect(0, 0, 256, 256);
    const glowMat = new T.SpriteMaterial({ map: new T.CanvasTexture(gc), blending: T.AdditiveBlending, depthWrite: false, transparent: true, opacity: 0.6 });
    const glow = new T.Sprite(glowMat);
    glow.scale.set(8, 8, 1);
    glow.position.z = -0.8;
    root.add(glow);

    const N = 256;
    const wg = new T.BufferGeometry();
    const wp = new Float32Array(N * 3);
    wg.setAttribute('position', new T.BufferAttribute(wp, 3));
    const waveMat = new T.LineBasicMaterial({ color: 0xffc27a, transparent: true, opacity: 0.8, blending: T.AdditiveBlending });
    const wave = new T.LineLoop(wg, waveMat);
    wave.position.z = -0.6;
    root.add(wave);

    const P = 1800;
    const pg = new T.BufferGeometry();
    const pa = new Float32Array(P * 3);
    for (let i = 0; i < P; i++) {
      const r = 2.6 + Math.random() * 4;
      const th = Math.random() * Math.PI * 2;
      const ph = Math.acos(2 * Math.random() - 1);
      pa[i * 3] = r * Math.sin(ph) * Math.cos(th);
      pa[i * 3 + 1] = r * Math.sin(ph) * Math.sin(th) * 0.6;
      pa[i * 3 + 2] = r * Math.cos(ph);
    }
    pg.setAttribute('position', new T.BufferAttribute(pa, 3));
    const dustMat = new T.PointsMaterial({ color: 0xffd2a0, size: 0.02, transparent: true, opacity: 0.5, blending: T.AdditiveBlending, depthWrite: false });
    const dust = new T.Points(pg, dustMat);
    root.add(dust);

    S.add(new T.AmbientLight(0xffffff, 0.06));
    const key = new T.PointLight(0xff8a3d, 70, 30, 1.4);
    key.position.set(-4, 2.5, 4);
    S.add(key);
    const rim = new T.PointLight(0x7fd4ff, 55, 30, 1.4);
    rim.position.set(4, -1, -2);
    S.add(rim);
    const top = new T.PointLight(0xffffff, 22, 30, 1.4);
    top.position.set(1, 5, 3);
    S.add(top);

    const bursts: Burst[] = [];
    this.ro.observe(this.el);
    this.resize();

    let t0 = performance.now();
    let lv = 0;
    let spk = 0;
    let dim = 1;
    let pulseCooldown = 0;
    const tg0 = sceneTarget(useApp.getState());
    root.position.set(tg0.x, tg0.y, 0);
    root.scale.setScalar(tg0.s * 0.6);

    const loop = (now: number) => {
      if (this.disposed) return;
      this.raf = requestAnimationFrame(loop);
      const dt = Math.min(0.05, (now - t0) / 1000);
      t0 = now;
      const t = now / 1000;
      const s = useApp.getState();
      const tg = sceneTarget(s);
      const k = 1 - Math.exp(-dt * 2.4);
      root.position.x += (tg.x - root.position.x) * k;
      root.position.y += (tg.y - root.position.y) * k;
      root.scale.setScalar(root.scale.x + (tg.s - root.scale.x) * k);
      dim += (tg.dim - dim) * k;

      const live = isLive(s) && !s.paused;
      lv += ((s.level || 0) - lv) * Math.min(1, dt * 10);
      spk += ((s.speaking ? 1 : 0) - spk) * Math.min(1, dt * 3);

      hs.rotation.y = Math.sin(t * 0.35) * 0.6 + 0.25;
      hs.rotation.x = 0.14 + Math.sin(t * 0.27) * 0.05;
      hs.position.y = Math.sin(t * 0.8) * 0.04;
      led.emissiveIntensity = (0.45 + spk * 2.4 + lv * 0.9) * dim;
      grill.opacity = (0.15 + spk * 0.35) * dim;
      cups.forEach((cp) => cp.g.scale.setScalar(1 + lv * 0.04));

      pulseCooldown -= dt;
      if (live && spk > 0.35 && pulseCooldown <= 0) {
        pulseCooldown = 0.2;
        sceneFx.push('pulse');
      }

      words.forEach((w, i) => {
        const a = w.ph * Math.PI * 2 + t * 0.11;
        const x = Math.cos(a) * 2.45;
        const z = Math.sin(a) * 1.3;
        const y = Math.sin(a * 2 + i) * 0.35 + 0.15;
        w.a.position.set(x, y, z);
        w.b.position.set(x, y, z);
        const p = (t / 7 + w.ph) % 1;
        const depth = 0.35 + (0.65 * (z / 1.3 + 1)) / 2;
        const base = 0.9 * dim * depth;
        const sa = p < 0.06 ? p / 0.06 : p < 0.4 ? 1 : p < 0.5 ? 1 - (p - 0.4) / 0.1 : 0;
        const sb = p < 0.4 ? 0 : p < 0.5 ? (p - 0.4) / 0.1 : p < 0.9 ? 1 : 1 - (p - 0.9) / 0.1;
        w.a.material.opacity = sa * base * 0.8;
        w.b.material.opacity = sb * base;
      });

      dust.rotation.y += dt * 0.018;
      dustMat.opacity = 0.5 * dim;
      glowMat.opacity = (0.4 + spk * 0.55 + lv * 0.15) * dim;

      for (let i = 0; i < N; i++) {
        const a = (i / N) * Math.PI * 2;
        const n = Math.sin(a * 6 + t * 3) * 0.5 + Math.sin(a * 11 - t * 5) * 0.3 + Math.sin(a * 23 + t * 7) * 0.2;
        const rr = 1.62 + (live ? lv * 0.24 * n + 0.01 * Math.sin(a * 40 + t * 9) : 0.004 * Math.sin(a * 8 + t));
        wp[i * 3] = Math.cos(a) * rr;
        wp[i * 3 + 1] = Math.sin(a) * rr;
        wp[i * 3 + 2] = 0;
      }
      wg.attributes.position.needsUpdate = true;
      waveMat.opacity = (live ? 0.85 : 0.22) * dim;

      while (sceneFx.length) {
        const kd = sceneFx.shift()!;
        const col = kd === 'ice' ? 0x7fd4ff : kd === 'pulse' ? 0xffb36a : 0xffa24a;
        const op = kd === 'pulse' ? 0.5 : 0.95;
        for (const cp of cups) {
          const m = new T.Mesh(
            new T.TorusGeometry(0.42, kd === 'pulse' ? 0.006 : 0.012, 8, 96),
            new T.MeshBasicMaterial({ color: col, transparent: true, opacity: op, blending: T.AdditiveBlending, depthWrite: false }),
          );
          m.rotation.y = Math.PI / 2;
          m.position.set(cp.sd * 1.4, -0.66, 0);
          hs.add(m);
          bursts.push({ m, age: 0, sd: cp.sd, op, life: kd === 'pulse' ? 1.1 : 1.6 });
        }
      }
      for (let i = bursts.length - 1; i >= 0; i--) {
        const b = bursts[i];
        b.age += dt;
        const q = b.age / b.life;
        b.m.scale.setScalar(1 + q * 1.6);
        b.m.position.x = b.sd * (1.4 + q * 1.1);
        b.m.material.opacity = b.op * (1 - q) * (1 - q) * dim;
        if (q >= 1) {
          hs.remove(b.m);
          b.m.geometry.dispose();
          b.m.material.dispose();
          bursts.splice(i, 1);
        }
      }

      C.position.x = Math.sin(t * 0.12) * 0.3;
      C.position.y = Math.cos(t * 0.1) * 0.18;
      C.lookAt(0, 0, 0);
      this.renderer.render(S, C);
    };
    this.raf = requestAnimationFrame(loop);
  }

  dispose() {
    this.disposed = true;
    cancelAnimationFrame(this.raf);
    this.ro.disconnect();
    this.renderer.dispose();
    this.renderer.domElement.remove();
  }
}
