// A real Crosstalk window in 3D: screenshot texture, rounded glass, soft shadow,
// and a reflection on the floor (mirrored across y = floorY).
import * as THREE from 'three';
import { windowMaterial, cardMaterial } from './materials.js';
import { canvas, roundRect, texture } from './text.js';

export const WIN_W = 6.4;
export const WIN_H = WIN_W / (2640 / 1680);

let shadowTex = null;
function shadowTexture(renderer) {
  if (shadowTex) return shadowTex;
  const { c, ctx } = canvas(800, 520);
  ctx.shadowColor = 'rgba(0,0,0,1)';
  ctx.shadowBlur = 60;
  ctx.fillStyle = 'rgba(0,0,0,0.9)';
  roundRect(ctx, 90, 90, 620, 340, 30);
  ctx.fill();
  shadowTex = texture(c, renderer);
  return shadowTex;
}

export class AppWindow {
  constructor(map, renderer, parent, { floorY = -2.3, w = WIN_W, h = WIN_H, radius = 0.1 } = {}) {
    this.floorY = floorY;
    this.root = new THREE.Group();
    const geo = new THREE.PlaneGeometry(w, h);
    this.mat = windowMaterial(map, { width: w, height: h, radius });
    this.mesh = new THREE.Mesh(geo, this.mat);
    this.shadow = new THREE.Mesh(new THREE.PlaneGeometry(w * 1.3, h * 1.5), cardMaterial(shadowTexture(renderer)));
    this.shadow.position.set(0.15, -0.35, -0.08);
    this.root.add(this.shadow, this.mesh);
    this.mmat = windowMaterial(map, { width: w, height: h, radius, mirror: true });
    this.mirror = new THREE.Mesh(geo, this.mmat);
    parent.add(this.root, this.mirror);
  }
  /** x, y, z, rotY, scale, opacity, dim, sweep */
  set({ x = 0, y = 0, z = 0, ry = 0, rx = 0, s = 1, opacity = 1, dim = 1, sweep = -1, reflect = 1, shadow = 1 }) {
    this.root.position.set(x, y, z);
    this.root.rotation.set(rx, ry, 0);
    this.root.scale.setScalar(s);
    this.mat.uniforms.uOpacity.value = opacity;
    this.mat.uniforms.uDim.value = dim;
    this.mat.uniforms.uSweep.value = sweep;
    this.shadow.material.uniforms.uOpacity.value = opacity * 0.75 * shadow;
    this.mirror.position.set(x, 2 * this.floorY - y, z);
    this.mirror.rotation.set(-rx, ry, 0);
    this.mirror.scale.set(s, -s, s);
    this.mmat.uniforms.uOpacity.value = opacity * reflect;
    this.mmat.uniforms.uDim.value = dim;
    this.mmat.uniforms.uSweep.value = -1;
    const vis = opacity > 0.002;
    this.root.visible = vis;
    this.mirror.visible = vis && reflect > 0.001;
  }
}
