// Shaders of the film. All transparent materials take a uOpacity uniform that the
// scenes drive every frame; nothing animates on its own.
import * as THREE from 'three';

/** Soft round particles, additive. Per-point colour, size and alpha; fake depth of field. */
export function pointsMaterial({ additive = true } = {}) {
  return new THREE.ShaderMaterial({
    transparent: true,
    depthWrite: false,
    blending: additive ? THREE.AdditiveBlending : THREE.NormalBlending,
    uniforms: {
      uOpacity: { value: 1 },
      uScale: { value: 1713 },
      uFocus: { value: 8 },
      uDof: { value: 0.0 },
      uCam: { value: new THREE.Vector3() },
      uWrap: { value: new THREE.Vector3(0, 0, 0) },
    },
    vertexShader: /* glsl */ `
      attribute vec3 aColor;
      attribute float aSize;
      attribute float aAlpha;
      uniform float uScale, uFocus, uDof;
      uniform vec3 uCam, uWrap;
      varying vec3 vColor;
      varying float vAlpha;
      varying float vSoft;
      void main() {
        vec3 p = position;
        if (uWrap.x > 0.0) p = mod(p - uCam + uWrap * 0.5, uWrap) - uWrap * 0.5 + uCam;
        vec4 mv = modelViewMatrix * vec4(p, 1.0);
        float d = -mv.z;
        float blur = min(uDof * abs(d - uFocus) / max(uFocus, 0.001), 0.9);
        gl_PointSize = clamp(aSize * uScale / d * (1.0 + blur * 2.5), 1.0, 22.0);
        vColor = aColor;
        vAlpha = aAlpha / (1.0 + blur * blur * 5.0);
        vSoft = clamp(0.35 + blur, 0.35, 1.0);
        gl_Position = projectionMatrix * mv;
      }`,
    fragmentShader: /* glsl */ `
      uniform float uOpacity;
      varying vec3 vColor;
      varying float vAlpha;
      varying float vSoft;
      void main() {
        vec2 c = gl_PointCoord - 0.5;
        float r = length(c) * 2.0;
        float a = 1.0 - smoothstep(1.0 - vSoft, 1.0, r);
        a *= vAlpha * uOpacity;
        if (a < 0.002) discard;
        gl_FragColor = vec4(vColor, a);
      }`,
  });
}

/** Thin glowing strip (a waveform line). Geometry is rebuilt on the CPU each frame. */
export function ribbonMaterial() {
  return new THREE.ShaderMaterial({
    transparent: true,
    depthWrite: false,
    blending: THREE.AdditiveBlending,
    side: THREE.DoubleSide,
    uniforms: { uOpacity: { value: 1 } },
    vertexShader: /* glsl */ `
      attribute vec3 aColor;
      attribute float aAlpha;
      attribute float aSide;
      varying vec3 vColor;
      varying float vAlpha;
      varying float vSide;
      void main() {
        vColor = aColor; vAlpha = aAlpha; vSide = aSide;
        gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0);
      }`,
    fragmentShader: /* glsl */ `
      uniform float uOpacity;
      varying vec3 vColor;
      varying float vAlpha;
      varying float vSide;
      void main() {
        float core = exp(-vSide * vSide * 9.0);
        float a = core * vAlpha * uOpacity;
        gl_FragColor = vec4(vColor, a);
      }`,
  });
}

/**
 * An application window: rounded corners, thin rim, light sweep, dimming,
 * and a mirrored mode for the floor reflection.
 */
export function windowMaterial(map, { width, height, radius = 0.09, mirror = false }) {
  return new THREE.ShaderMaterial({
    transparent: true,
    depthWrite: false,
    side: THREE.DoubleSide,
    uniforms: {
      map: { value: map },
      map2: { value: map },
      uMix: { value: 0 },
      uOpacity: { value: 1 },
      uDim: { value: 1 },
      uSweep: { value: -1 },
      uSize: { value: new THREE.Vector2(width, height) },
      uRadius: { value: radius },
      uMirror: { value: mirror ? 1 : 0 },
    },
    vertexShader: /* glsl */ `
      varying vec2 vUv;
      void main() { vUv = uv; gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0); }`,
    fragmentShader: /* glsl */ `
      uniform sampler2D map, map2;
      uniform float uMix, uOpacity, uDim, uSweep, uRadius, uMirror;
      uniform vec2 uSize;
      varying vec2 vUv;
      float rbox(vec2 p, vec2 b, float r) { vec2 q = abs(p) - b + r; return length(max(q, 0.0)) + min(max(q.x, q.y), 0.0) - r; }
      void main() {
        vec2 p = (vUv - 0.5) * uSize;
        float d = rbox(p, uSize * 0.5, uRadius);
        float aa = fwidth(d) * 1.2;
        float inside = 1.0 - smoothstep(-aa, aa, d);
        if (inside < 0.001) discard;
        vec4 tex = mix(texture2D(map, vUv), texture2D(map2, vUv), uMix);
        vec3 col = tex.rgb * uDim;
        // Rim: a hairline that catches the light, brighter on the top edge.
        float rim = 1.0 - smoothstep(0.0, aa * 2.5, abs(d + aa));
        col += rim * mix(0.05, 0.16, vUv.y);
        // Light sweep: a soft diagonal band crossing the glass.
        float band = (vUv.x * 0.8 + (1.0 - vUv.y) * 0.45) - uSweep;
        col += vec3(1.0, 0.86, 0.72) * exp(-band * band * 90.0) * 0.10;
        float a = inside * uOpacity;
        if (uMirror > 0.5) {
          // Reflection on a dark floor: only the bottom part, quickly fading.
          a *= 0.2 * (1.0 - smoothstep(0.0, 0.42, vUv.y));
          col *= 0.8;
        }
        gl_FragColor = vec4(col, a);
      }`,
  });
}

/** A textured card (canvas texture) with optional blurred twin for depth of field. */
export function cardMaterial(map, blurMap) {
  return new THREE.ShaderMaterial({
    transparent: true,
    depthWrite: false,
    side: THREE.DoubleSide,
    uniforms: {
      map: { value: map },
      blurMap: { value: blurMap ?? map },
      uBlur: { value: 0 },
      uOpacity: { value: 1 },
      uReveal: { value: 2 },
      uTint: { value: new THREE.Vector3(1, 1, 1) },
    },
    vertexShader: /* glsl */ `
      varying vec2 vUv;
      void main() { vUv = uv; gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0); }`,
    fragmentShader: /* glsl */ `
      uniform sampler2D map, blurMap;
      uniform float uBlur, uOpacity, uReveal;
      uniform vec3 uTint;
      varying vec2 vUv;
      void main() {
        vec4 c = mix(texture2D(map, vUv), texture2D(blurMap, vUv), clamp(uBlur, 0.0, 1.0));
        float rev = 1.0 - smoothstep(uReveal - 0.04, uReveal, vUv.x);
        float a = c.a * uOpacity * rev;
        if (a < 0.002) discard;
        gl_FragColor = vec4(c.rgb * uTint, a);
      }`,
  });
}

/** Full-frame backdrop: ink, warm ember glow and a cold ice glow (as in the README cover). */
export function backdropMaterial() {
  return new THREE.ShaderMaterial({
    depthWrite: false,
    depthTest: false,
    uniforms: {
      uEmber: { value: 0.2 },
      uIce: { value: 0.1 },
      uEmberPos: { value: new THREE.Vector2(0.7, 0.55) },
      uIcePos: { value: new THREE.Vector2(0.95, 0.15) },
      uLift: { value: 1 },
      uTime: { value: 0 },
    },
    vertexShader: /* glsl */ `
      varying vec2 vUv;
      void main() { vUv = uv; gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0); }`,
    fragmentShader: /* glsl */ `
      uniform float uEmber, uIce, uLift, uTime;
      uniform vec2 uEmberPos, uIcePos;
      varying vec2 vUv;
      void main() {
        vec2 a = vec2(16.0 / 9.0, 1.0);
        vec3 base = mix(vec3(0.0060, 0.0050, 0.0060), vec3(0.0006), smoothstep(0.0, 1.1, length((vUv - vec2(0.4, 0.6)) * a)));
        float e = exp(-pow(length((vUv - uEmberPos) * a) / 0.55, 2.0));
        float i = exp(-pow(length((vUv - uIcePos) * a) / 0.6, 2.0));
        vec3 col = base * uLift + vec3(1.0, 0.36, 0.068) * e * uEmber * 0.022 + vec3(0.21, 0.66, 1.0) * i * uIce * 0.016;
        gl_FragColor = vec4(col, 1.0);
      }`,
  });
}

/** Infinite tactical floor: fine grid fading into the dark, with listening rings. */
export function floorMaterial() {
  return new THREE.ShaderMaterial({
    transparent: true,
    depthWrite: false,
    uniforms: { uOpacity: { value: 1 }, uTime: { value: 0 }, uRings: { value: 1 } },
    vertexShader: /* glsl */ `
      varying vec3 vW;
      void main() { vec4 w = modelMatrix * vec4(position, 1.0); vW = w.xyz; gl_Position = projectionMatrix * viewMatrix * w; }`,
    fragmentShader: /* glsl */ `
      uniform float uOpacity, uTime, uRings;
      varying vec3 vW;
      float grid(vec2 p, float s) { vec2 g = abs(fract(p / s - 0.5) - 0.5) / fwidth(p / s); return 1.0 - min(min(g.x, g.y), 1.0); }
      void main() {
        vec2 p = vW.xz;
        float fade = exp(-pow(length(p - vec2(0.0, -3.0)) / 13.0, 2.0));
        float g = grid(p, 1.0) * 0.05 + grid(p, 5.0) * 0.07;
        vec3 col = vec3(0.89, 0.85, 0.77) * g;
        // Expanding rings: a voice being heard somewhere on the map.
        float rings = 0.0;
        vec2 cs[3]; cs[0] = vec2(-4.5, -4.0); cs[1] = vec2(5.0, -6.5); cs[2] = vec2(0.5, -1.5);
        for (int k = 0; k < 3; k++) {
          float d = length(p - cs[k]);
          float ph = fract(uTime * 0.35 + float(k) * 0.33);
          float r = ph * 5.0;
          rings += exp(-pow((d - r) * 6.0, 2.0)) * (1.0 - ph) * 0.5;
        }
        col += vec3(1.0, 0.36, 0.068) * rings * uRings;
        float a = (g + rings * uRings) * fade * uOpacity;
        gl_FragColor = vec4(col * fade, clamp(a, 0.0, 1.0));
      }`,
  });
}
