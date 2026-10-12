// The installer's 3D stage (headset, beam, halos, particles, signal lines),
// ported from the Claude Design file "Crosstalk Installer".

import * as T from 'three';

export interface StageState {
  screen: number;
  prog: number;
  reducedMotion: boolean;
  mx: number;
  my: number;
  introStart: number;
}

/** Per-screen pose and light levels (design: TG). */
const TG = [
  { x: 1.55, y: 0.02, s: 0.95, ry: -0.55, rx: 0.1, cz: 7, beam: 1, glow: 0.45, pOp: 0.5, w: [1, 0, 0] },
  { x: 1.8, y: 0.1, s: 0.86, ry: -0.98, rx: 0.14, cz: 7.3, beam: 0.5, glow: 0.26, pOp: 0.3, w: [1, 0, 0] },
  { x: 0, y: 0.5, s: 0.84, ry: 0, rx: 0.06, cz: 7.6, beam: 0.8, glow: 0.7, pOp: 1, w: [0, 1, 0] },
  { x: 0, y: 0.78, s: 0.74, ry: 0, rx: -0.05, cz: 7.2, beam: 1, glow: 1.15, pOp: 0.9, w: [0, 0, 1] },
];

export interface Stage {
  /** Velocity kick when the screen changes (spin on completion, lift during install). */
  kick(screen: number): void;
  dispose(): void;
}

export function createStage(el: HTMLElement, get: () => StageState): Stage | null {
  let R: T.WebGLRenderer;
  try {
    R = new T.WebGLRenderer({ antialias: true, alpha: true, powerPreference: 'high-performance' });
  } catch {
    return null;
  }
  const W = 1000,
    H = 640;
  R.setPixelRatio(Math.min(2, devicePixelRatio || 1));
  R.setSize(W, H);
  R.toneMapping = T.ACESFilmicToneMapping;
  R.toneMappingExposure = 1.05;
  R.outputColorSpace = T.SRGBColorSpace;
  R.domElement.style.cssText = 'position:absolute;inset:0;width:100%;height:100%;display:block';
  el.appendChild(R.domElement);
  const scene = new T.Scene();
  scene.fog = new T.FogExp2(0x08090c, 0.045);
  const cam = new T.PerspectiveCamera(30, W / H, 0.1, 100);
  cam.position.set(0, 0.2, 7);

  const pm = new T.PMREMGenerator(R),
    es = new T.Scene();
  es.add(new T.Mesh(new T.BoxGeometry(24, 24, 24), new T.MeshBasicMaterial({ color: 0x050506, side: T.BackSide })));
  const panel = (c: number, i: number, x: number, y: number, z: number, w: number, h: number) => {
    const m = new T.Mesh(new T.PlaneGeometry(w, h), new T.MeshBasicMaterial({ color: new T.Color(c).multiplyScalar(i), side: T.DoubleSide }));
    m.position.set(x, y, z);
    m.lookAt(0, 0, 0);
    es.add(m);
  };
  panel(0xffffff, 2.6, 0, 9, 2, 9, 3);
  panel(0xff643b, 3.2, -8, 1, -3, 3, 9);
  panel(0x91dceb, 2, 8, 0.5, -2, 2.4, 8);
  panel(0xffffff, 0.9, 0, 1, 10, 12, 2.5);
  scene.environment = pm.fromScene(es, 0.04).texture;

  const shell = new T.MeshPhysicalMaterial({ color: 0x0f1013, roughness: 0.5, metalness: 0.3, clearcoat: 0.55, clearcoatRoughness: 0.4, envMapIntensity: 0.9 });
  const soft = new T.MeshPhysicalMaterial({ color: 0x0a0a0c, roughness: 0.92, metalness: 0, sheen: 1, sheenRoughness: 0.8, sheenColor: new T.Color(0x2a2b31) });
  const metal = new T.MeshStandardMaterial({ color: 0xb9bec6, roughness: 0.22, metalness: 1, envMapIntensity: 1.2 });
  const gun = new T.MeshStandardMaterial({ color: 0x2b2d33, roughness: 0.32, metalness: 0.95 });
  const ember = new T.MeshStandardMaterial({ color: 0xff643b, emissive: 0xff643b, emissiveIntensity: 2.4, toneMapped: false });
  const ice = new T.MeshStandardMaterial({ color: 0x91dceb, emissive: 0x91dceb, emissiveIntensity: 1.3, toneMapped: false });

  const root = new T.Group(),
    hs = new T.Group();
  root.add(hs);
  scene.add(root);
  const band = new T.Mesh(new T.TorusGeometry(1, 0.058, 28, 160, Math.PI), shell);
  band.scale.z = 2.7;
  hs.add(band);
  const pad = new T.Mesh(new T.TorusGeometry(0.925, 0.05, 20, 120, Math.PI * 0.8), soft);
  pad.rotation.z = Math.PI * 0.1;
  pad.scale.z = 2.1;
  hs.add(pad);
  [-0.11, 0.11].forEach((z) => {
    const l = new T.Mesh(new T.TorusGeometry(1.056, 0.0075, 8, 140, Math.PI * 0.46), ember);
    l.rotation.z = Math.PI * 0.27;
    l.position.z = z;
    hs.add(l);
  });
  const crown = new T.Mesh(new T.BoxGeometry(0.34, 0.03, 0.2), gun);
  crown.position.y = 1.06;
  hs.add(crown);

  [-1, 1].forEach((s) => {
    const cup = new T.Group();
    cup.position.set(s * 1.03, -0.56, 0);
    cup.scale.x = s;
    hs.add(cup);
    const fork = new T.Mesh(new T.TorusGeometry(0.53, 0.026, 14, 64, Math.PI), metal);
    fork.rotation.y = Math.PI / 2;
    fork.position.x = 0.02;
    cup.add(fork);
    const hub = new T.Mesh(new T.CylinderGeometry(0.065, 0.065, 0.12, 32), gun);
    hub.position.set(0.02, 0.55, 0);
    cup.add(hub);
    const rod = new T.Mesh(new T.CylinderGeometry(0.022, 0.022, 0.26, 16), metal);
    rod.position.set(-0.02, 0.66, 0);
    cup.add(rod);
    [-1, 1].forEach((z) => {
      const p = new T.Mesh(new T.SphereGeometry(0.045, 20, 16), gun);
      p.position.set(0.02, 0, z * 0.53);
      cup.add(p);
    });
    const body = new T.Mesh(new T.CylinderGeometry(0.47, 0.41, 0.3, 72), shell);
    body.rotation.z = Math.PI / 2;
    cup.add(body);
    const plate = new T.Mesh(new T.CylinderGeometry(0.36, 0.36, 0.04, 72), gun);
    plate.rotation.z = Math.PI / 2;
    plate.position.x = 0.16;
    cup.add(plate);
    const emb = new T.Mesh(new T.CylinderGeometry(0.13, 0.13, 0.025, 48), shell);
    emb.rotation.z = Math.PI / 2;
    emb.position.x = 0.185;
    cup.add(emb);
    const ring = new T.Mesh(new T.TorusGeometry(0.295, 0.0085, 8, 120), ember);
    ring.rotation.y = Math.PI / 2;
    ring.position.x = 0.185;
    cup.add(ring);
    const tick = new T.Mesh(new T.TorusGeometry(0.43, 0.006, 8, 48, Math.PI * 0.3), ice);
    tick.rotation.y = Math.PI / 2;
    tick.rotation.x = Math.PI * 1.15;
    tick.position.x = 0.1;
    cup.add(tick);
    const cush = new T.Mesh(new T.TorusGeometry(0.33, 0.12, 24, 72), soft);
    cush.rotation.y = Math.PI / 2;
    cush.scale.z = 0.85;
    cush.position.x = -0.2;
    cup.add(cush);
    const fab = new T.Mesh(new T.CylinderGeometry(0.29, 0.29, 0.02, 48), soft);
    fab.rotation.z = Math.PI / 2;
    fab.position.x = -0.24;
    cup.add(fab);
  });
  const curve = new T.CatmullRomCurve3([new T.Vector3(-1.14, -0.74, 0.2), new T.Vector3(-1.08, -0.98, 0.62), new T.Vector3(-0.78, -1.1, 0.98), new T.Vector3(-0.36, -1.08, 1.12)]);
  hs.add(new T.Mesh(new T.TubeGeometry(curve, 80, 0.022, 12), gun));
  const piv = new T.Mesh(new T.SphereGeometry(0.07, 24, 16), shell);
  piv.position.copy(curve.points[0]);
  hs.add(piv);
  const tipPos = curve.getPoint(1),
    tan = curve.getTangent(1);
  const tip = new T.Mesh(new T.CapsuleGeometry(0.05, 0.12, 8, 20), shell);
  tip.position.copy(tipPos);
  tip.quaternion.setFromUnitVectors(new T.Vector3(0, 1, 0), tan);
  hs.add(tip);
  const tipL = new T.Mesh(new T.TorusGeometry(0.052, 0.007, 8, 40), ember);
  tipL.position.copy(tipPos).addScaledVector(tan, -0.04);
  tipL.quaternion.setFromUnitVectors(new T.Vector3(0, 0, 1), tan);
  hs.add(tipL);

  scene.add(new T.AmbientLight(0x1c1e24, 0.5));
  const key = new T.SpotLight(0xfff1e6, 0, 22, Math.PI / 7, 0.65, 1.1);
  key.position.set(0.4, 6.5, 3.2);
  scene.add(key, key.target);
  const rimO = new T.PointLight(0xff643b, 0, 14, 1.5);
  rimO.position.set(-3, 1.2, -2.6);
  scene.add(rimO);
  const rimB = new T.PointLight(0x91dceb, 0, 14, 1.5);
  rimB.position.set(3.4, -0.4, -2.2);
  scene.add(rimB);
  const fill = new T.PointLight(0xffffff, 0, 12, 1.8);
  fill.position.set(0, -0.4, 4.5);
  scene.add(fill);

  const beamMat = new T.ShaderMaterial({
    transparent: true,
    depthWrite: false,
    blending: T.AdditiveBlending,
    side: T.DoubleSide,
    uniforms: { uI: { value: 0 }, uC: { value: new T.Color(0xffe2cf) } },
    vertexShader: 'varying vec2 vUv;varying vec3 vN;void main(){vUv=uv;vN=normalize(normalMatrix*normal);gl_Position=projectionMatrix*modelViewMatrix*vec4(position,1.);}',
    fragmentShader: 'uniform float uI;uniform vec3 uC;varying vec2 vUv;varying vec3 vN;void main(){float f=pow(abs(vN.z),2.2);float a=pow(vUv.y,1.4)*.16*f*uI;gl_FragColor=vec4(uC,a);}',
  });
  const beam = new T.Mesh(new T.ConeGeometry(2.3, 8, 64, 1, true), beamMat);
  beam.position.y = 2.9;
  scene.add(beam);

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
        [0.35, 'rgba(255,100,59,.35)'],
        [1, 'rgba(255,100,59,0)'],
      ]),
      blending: T.AdditiveBlending,
      depthWrite: false,
      transparent: true,
      opacity: 0,
    }),
  );
  glow.scale.set(6.5, 6.5, 1);
  scene.add(glow);
  const floor = new T.Mesh(new T.PlaneGeometry(40, 40), new T.MeshStandardMaterial({ color: 0x0a0b0e, roughness: 0.42, metalness: 0.7 }));
  floor.rotation.x = -Math.PI / 2;
  floor.position.y = -1.85;
  scene.add(floor);
  const blob = new T.Mesh(
    new T.PlaneGeometry(3.4, 1.6),
    new T.MeshBasicMaterial({
      map: ctex(128, [
        [0, 'rgba(0,0,0,.85)'],
        [1, 'rgba(0,0,0,0)'],
      ]),
      transparent: true,
      depthWrite: false,
    }),
  );
  blob.rotation.x = -Math.PI / 2;
  blob.position.y = -1.84;
  scene.add(blob);
  const halo = new T.Mesh(new T.TorusGeometry(1.34, 0.0045, 8, 220), new T.MeshBasicMaterial({ color: 0xff643b, transparent: true, opacity: 0, blending: T.AdditiveBlending, depthWrite: false }));
  scene.add(halo);
  const halo2 = new T.Mesh(new T.TorusGeometry(1.44, 0.003, 8, 220), new T.MeshBasicMaterial({ color: 0x91dceb, transparent: true, opacity: 0, blending: T.AdditiveBlending, depthWrite: false }));
  scene.add(halo2);

  const N = 1500,
    pos = new Float32Array(N * 3),
    col = new Float32Array(N * 3),
    B = new Float32Array(N * 3),
    P = new Float32Array(N * 4);
  const cE = new T.Color(0xff643b),
    cI = new T.Color(0x91dceb),
    cW = new T.Color(0xf2f1ee);
  for (let i = 0; i < N; i++) {
    B[i * 3] = (Math.random() - 0.5) * 12;
    B[i * 3 + 1] = (Math.random() - 0.5) * 6;
    B[i * 3 + 2] = (Math.random() - 0.5) * 5 - 1;
    const side = Math.random() < 0.5 ? -1 : 1;
    P[i * 4] = Math.random();
    P[i * 4 + 1] = side;
    P[i * 4 + 2] = Math.floor(Math.random() * 3);
    P[i * 4 + 3] = 0.7 + Math.random() * 0.6;
    const c = Math.random() < 0.12 ? cW : side < 0 ? cE : cI;
    col[i * 3] = c.r;
    col[i * 3 + 1] = c.g;
    col[i * 3 + 2] = c.b;
  }
  const pg = new T.BufferGeometry();
  pg.setAttribute('position', new T.BufferAttribute(pos, 3));
  pg.setAttribute('color', new T.BufferAttribute(col, 3));
  const pmat = new T.PointsMaterial({
    size: 0.034,
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
  const LP = 180,
    lines: { l: T.Line; side: number; lane: number }[] = [];
  for (let i = 0; i < 6; i++) {
    const g = new T.BufferGeometry();
    g.setAttribute('position', new T.BufferAttribute(new Float32Array(LP * 3), 3));
    const m = new T.LineBasicMaterial({ color: i < 3 ? 0xff643b : 0x91dceb, transparent: true, opacity: 0, blending: T.AdditiveBlending, depthWrite: false });
    const l = new T.Line(g, m);
    lines.push({ l, side: i < 3 ? -1 : 1, lane: i % 3 });
    scene.add(l);
  }

  const mk = (v: number) => ({ x: v, v: 0 });
  const t0 = TG[get().screen];
  const sp = { x: mk(t0.x + 0.6), y: mk(t0.y - 0.3), s: mk(t0.s * 0.9), ry: mk(t0.ry - 1.6), rx: mk(t0.rx), cz: mk(t0.cz - 1.4) };
  type Key = keyof typeof sp;
  const lv = { beam: 0, glow: 0, pOp: 0, w0: t0.w[0], w1: t0.w[1], w2: t0.w[2] };
  const sm = (a: number, b: number, x: number) => {
    const t = Math.min(1, Math.max(0, (x - a) / (b - a)));
    return t * t * (3 - 2 * t);
  };
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
      tg = TG[st.screen];
    tt += dt * (rm ? 0.15 : 1);
    const ie = (now - st.introStart) / 1000;
    const kB = sm(0.2, 2.2, ie),
      kK = sm(0.6, 2.8, ie),
      kR = sm(1.4, 3.2, ie);
    const tgt: Record<Key, number> = {
      x: tg.x,
      y: tg.y + (rm ? 0 : Math.sin(tt * 0.9) * (st.screen === 2 ? 0.07 : 0.04)),
      s: tg.s,
      ry: tg.ry - (1 - kK) * 1.6 + (rm ? 0 : Math.sin(tt * 0.32) * (st.screen === 1 ? 0.1 : 0.2)),
      rx: tg.rx,
      cz: tg.cz - (1 - kK) * 1.4,
    };
    const k = rm ? 60 : 13,
      d = rm ? 15.5 : 6.2;
    for (const n of Object.keys(sp) as Key[]) {
      const o = sp[n];
      o.v += ((tgt[n] - o.x) * k - o.v * d) * dt;
      o.x += o.v * dt;
    }
    const f = 1 - Math.exp(-dt * 2.4);
    lv.beam += (tg.beam - lv.beam) * f;
    lv.glow += (tg.glow * (st.screen === 2 ? 0.7 + st.prog * 0.006 : 1) - lv.glow) * f;
    lv.pOp += (tg.pOp - lv.pOp) * f;
    lv.w0 += (tg.w[0] - lv.w0) * f;
    lv.w1 += (tg.w[1] - lv.w1) * f;
    lv.w2 += (tg.w[2] - lv.w2) * f;
    const gx = sp.x.x,
      gy = sp.y.x,
      gs = sp.s.x;
    root.position.set(gx, gy, 0);
    root.scale.setScalar(gs);
    root.rotation.set(sp.rx.x, sp.ry.x, 0);
    const orb = rm ? 0 : Math.sin(tt * 0.11) * 0.12 + st.mx * 0.06;
    cam.position.set(Math.sin(orb) * sp.cz.x, 0.2 + (rm ? 0 : -st.my * 0.12), Math.cos(orb) * sp.cz.x);
    cam.lookAt(gx * 0.25, 0.1, 0);
    key.intensity = 140 * kK * (0.55 + lv.beam * 0.45);
    key.target.position.set(gx, gy, 0);
    key.position.x = gx * 0.9 + 0.4;
    rimO.intensity = 38 * kR;
    rimB.intensity = 26 * kR;
    fill.intensity = 4 * kK;
    beamMat.uniforms.uI.value = kB * lv.beam;
    beam.position.x = gx * 0.92;
    glow.position.set(gx, gy, -1.3);
    glow.material.opacity = lv.glow * 0.5 * kR;
    blob.position.x = gx;
    blob.material.opacity = 0.8 - Math.min(0.4, gy * 0.3);
    halo.position.set(gx, gy, -0.55);
    halo2.position.set(gx, gy, -0.6);
    halo.material.opacity = lv.w2 * 0.55;
    halo2.material.opacity = lv.w2 * 0.22;
    halo.rotation.z = tt * 0.05;
    halo2.rotation.z = -tt * 0.03;
    const prog = st.prog,
      wD = lv.w0,
      wS = lv.w1,
      wR = lv.w2,
      wt = wD + wS + wR || 1;
    const spdF = 0.16 + prog * 0.0035;
    for (let i = 0; i < N; i++) {
      const ph = P[i * 4],
        side = P[i * 4 + 1],
        lane = P[i * 4 + 2],
        spd = P[i * 4 + 3];
      let x = 0,
        y = 0,
        z = 0;
      if (wD > 0.002) {
        x += wD * (B[i * 3] + Math.sin(tt * 0.21 + ph * 6.28) * 0.18);
        y += wD * (B[i * 3 + 1] + Math.cos(tt * 0.17 + ph * 9) * 0.14);
        z += wD * B[i * 3 + 2];
      }
      if (wS > 0.002) {
        const u = (ph + tt * spd * spdF) % 1,
          cx = gx + side * 1.03 * gs,
          cy = gy - 0.56 * gs,
          x0 = side * 6.4;
        let sx, sy, sz;
        if (u < 0.82) {
          const q = u / 0.82,
            e = 1 - q;
          sx = x0 + (cx + side * 0.3 * gs - x0) * q;
          sy = cy + (lane - 1) * 0.32 * e + Math.sin(sx * 2.4 - tt * 3.2 + lane * 1.3) * 0.3 * e * (0.6 + 0.4 * Math.sin(tt * 0.7 + lane));
          sz = (lane - 1) * 0.25 * e + Math.sin(ph * 40) * 0.04;
        } else {
          const q = (u - 0.82) / 0.18,
            a = ph * 6.283 + tt * 2.2 * side,
            r = (0.5 - 0.2 * q) * gs;
          sx = cx + side * (0.3 - 0.25 * q) * gs;
          sy = cy + Math.cos(a) * r;
          sz = Math.sin(a) * r;
        }
        x += wS * sx;
        y += wS * sy;
        z += wS * sz;
      }
      if (wR > 0.002) {
        const a = ph * 6.283 + tt * 0.12 * (lane === 1 ? -1 : 1),
          r = 1.36 + Math.sin(ph * 97 + tt * 0.6) * 0.05 + (lane - 1) * 0.03;
        x += wR * (gx + Math.cos(a) * r);
        y += wR * (gy + Math.sin(a) * r);
        z += wR * (-0.55 + Math.sin(ph * 31) * 0.1);
      }
      pos[i * 3] = x / wt;
      pos[i * 3 + 1] = y / wt;
      pos[i * 3 + 2] = z / wt;
    }
    pg.attributes.position.needsUpdate = true;
    pmat.opacity = lv.pOp * kR * 0.9;
    pmat.size = 0.03 + wS * 0.012;
    lines.forEach(({ l, side, lane }) => {
      const a = l.geometry.attributes.position.array as Float32Array,
        cx = gx + side * 1.03 * gs,
        cy = gy - 0.56 * gs,
        x0 = side * 6.4;
      for (let j = 0; j < LP; j++) {
        const q = j / (LP - 1),
          e = 1 - q;
        const sx = x0 + (cx + side * 0.3 * gs - x0) * q;
        a[j * 3] = sx;
        a[j * 3 + 1] = cy + (lane - 1) * 0.32 * e + Math.sin(sx * 2.4 - tt * 3.2 + lane * 1.3) * 0.3 * e * (0.6 + 0.4 * Math.sin(tt * 0.7 + lane));
        a[j * 3 + 2] = (lane - 1) * 0.25 * e;
      }
      l.geometry.attributes.position.needsUpdate = true;
      (l.material as T.LineBasicMaterial).opacity = wS * (side < 0 ? 0.32 : 0.26);
    });
    R.render(scene, cam);
  };
  raf = requestAnimationFrame(loop);

  return {
    kick(screen) {
      if (get().reducedMotion) return;
      if (screen === 3) sp.ry.v += 7.5;
      if (screen === 2) sp.y.v += 1.2;
    },
    dispose() {
      dead = true;
      cancelAnimationFrame(raf);
      R.dispose();
      R.domElement.remove();
    },
  };
}
