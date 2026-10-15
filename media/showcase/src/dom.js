// Typography layer: real HTML text over the WebGL canvas (crisp fonts), driven per frame.
import { clamp, cine, range } from './util.js';

export function el(parent, html, style = {}, cls = '') {
  const d = document.createElement('div');
  if (cls) d.className = cls;
  d.innerHTML = html;
  Object.assign(d.style, { position: 'absolute', willChange: 'opacity, transform, filter' }, style);
  parent.appendChild(d);
  return d;
}

/** Wraps every character (or word) in a span so it can be revealed one by one. */
export function split(node, by = 'char') {
  const spans = [];
  const walk = (n) => {
    for (const child of [...n.childNodes]) {
      if (child.nodeType === 3) {
        const parts = by === 'word' ? child.textContent.split(/(\s+)/) : [...child.textContent];
        const frag = document.createDocumentFragment();
        for (const p of parts) {
          if (/^\s+$/.test(p)) {
            frag.appendChild(document.createTextNode(p));
            continue;
          }
          const s = document.createElement('span');
          s.textContent = p;
          s.style.display = 'inline-block';
          s.style.willChange = 'opacity, transform, filter';
          frag.appendChild(s);
          spans.push(s);
        }
        n.replaceChild(frag, child);
      } else walk(child);
    }
  };
  walk(node);
  return spans;
}

/** The app's reveal: fade up out of a blur (Intro.tsx), staggered. */
export function reveal(spans, t, start, stagger, dur = 0.9, { y = 18, blur = 10 } = {}) {
  spans.forEach((s, i) => {
    const k = cine(range(t, start + i * stagger, start + i * stagger + dur));
    s.style.opacity = k.toFixed(3);
    s.style.transform = `translateY(${((1 - k) * y).toFixed(2)}px)`;
    s.style.filter = k < 0.999 ? `blur(${((1 - k) * blur).toFixed(2)}px)` : 'none';
  });
}

/** Whole-block visibility with an optional blur-out and scale. */
export function show(node, a, { blur = 0, scale = 1, x = 0, y = 0 } = {}) {
  node.style.opacity = clamp(a).toFixed(3);
  node.style.visibility = a <= 0.001 ? 'hidden' : 'visible';
  node.style.transform = `translate(${x.toFixed(2)}px, ${y.toFixed(2)}px) scale(${scale.toFixed(4)})`;
  node.style.filter = blur > 0.01 ? `blur(${blur.toFixed(2)}px)` : 'none';
}
