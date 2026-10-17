import type { CSSProperties } from 'react';

const cache = new Map<string, CSSProperties>();

/** Inline CSS text (as in the design file) to a React style object. */
export function css(text: string): CSSProperties {
  const hit = cache.get(text);
  if (hit) return hit;
  const out: Record<string, string> = {};
  let depth = 0;
  let start = 0;
  const decls: string[] = [];
  for (let i = 0; i < text.length; i++) {
    const c = text[i];
    if (c === '(') depth++;
    else if (c === ')') depth--;
    else if (c === ';' && depth === 0) {
      decls.push(text.slice(start, i));
      start = i + 1;
    }
  }
  decls.push(text.slice(start));
  for (const d of decls) {
    const k = d.indexOf(':');
    if (k < 0) continue;
    const prop = d.slice(0, k).trim();
    // The design names the families; the bundled (offline) fonts register these names.
    const value = d.slice(k + 1).trim().replace(/'Archivo'/g, "'Archivo Variable'").replace(/'Geist'/g, "'Geist Sans'");
    if (!prop) continue;
    const name = prop.startsWith('--') ? prop : prop.replace(/^-(webkit|moz|ms)-/, (_, p: string) => p.charAt(0).toUpperCase() + p.slice(1) + '-').replace(/-([a-z])/g, (_, l: string) => l.toUpperCase());
    out[name] = value;
  }
  cache.set(text, out as CSSProperties);
  return out as CSSProperties;
}
