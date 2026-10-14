// Architecture rules, checked locally (no GitHub Actions). See
// docs/architecture/dependencies.md for the rules and their reasons.
//
//   node scripts/check-architecture.mjs           check
//   node scripts/check-architecture.mjs --write   refresh docs/architecture/ipc-contract.txt
//
// 1. Rust: no new cycle between the top-level modules of src-tauri/src.
// 2. Rust: the files the installer compiles with #[path] only use each other.
// 3. Frontend: no import cycle (type-only imports excluded), and the IPC layer
//    (src/shared/ipc) never imports the UI.
// 4. IPC contract: Tauri commands and event names match the committed list,
//    so a refactoring cannot rename one silently.
import { existsSync, readdirSync, readFileSync, statSync, writeFileSync } from 'node:fs';
import { dirname, join, normalize, relative } from 'node:path';

const errors = [];
const notes = [];

function* walk(dir) {
  for (const name of readdirSync(dir)) {
    const path = join(dir, name);
    if (statSync(path).isDirectory()) yield* walk(path);
    else yield path;
  }
}

/** Rust source without its trailing unit tests and without comments. */
function rustCode(path) {
  const text = readFileSync(path, 'utf8');
  const cut = text.search(/^#\[cfg\(test\)\]\s*\nmod tests/m);
  return (cut < 0 ? text : text.slice(0, cut))
    .split('\n')
    .filter((l) => !/^\s*\/\//.test(l))
    .join('\n');
}

/** Strongly connected components with more than one node (Tarjan). */
function cycles(graph) {
  let index = 0;
  const idx = new Map();
  const low = new Map();
  const stack = [];
  const on = new Set();
  const out = [];
  const visit = (v) => {
    idx.set(v, index);
    low.set(v, index++);
    stack.push(v);
    on.add(v);
    for (const w of graph.get(v) ?? []) {
      if (!idx.has(w)) {
        visit(w);
        low.set(v, Math.min(low.get(v), low.get(w)));
      } else if (on.has(w)) low.set(v, Math.min(low.get(v), idx.get(w)));
    }
    if (low.get(v) === idx.get(v)) {
      const scc = [];
      let w;
      do {
        w = stack.pop();
        on.delete(w);
        scc.push(w);
      } while (w !== v);
      if (scc.length > 1) out.push(scc.sort());
    }
  };
  for (const v of graph.keys()) if (!idx.has(v)) visit(v);
  return out;
}

// ---------- 1. Rust module graph ----------

const RUST = 'src-tauri/src';
// Known cycles, kept on purpose and documented in dependencies.md. A cycle
// that is not listed here (or grows) fails the check.
const KNOWN_RUST_CYCLES = [];

const modules = new Map();
for (const name of readdirSync(RUST)) {
  const path = join(RUST, name);
  const mod = name.replace(/\.rs$/, '');
  if (['lib', 'main'].includes(mod)) continue;
  if (statSync(path).isDirectory()) modules.set(mod, [...walk(path)].filter((p) => p.endsWith('.rs')));
  else if (name.endsWith('.rs')) modules.set(mod, [path]);
}
const rustGraph = new Map();
for (const [mod, files] of modules) {
  const deps = new Set();
  for (const f of files) {
    const code = rustCode(f);
    for (const m of code.matchAll(/\bcrate::(\w+)/g)) if (m[1] !== mod && modules.has(m[1])) deps.add(m[1]);
    for (const m of code.matchAll(/\bcrate::\{([^}]*)\}/g))
      for (const part of m[1].split(','))
        if (modules.has(part.trim().split('::')[0]) && part.trim().split('::')[0] !== mod) deps.add(part.trim().split('::')[0]);
  }
  rustGraph.set(mod, deps);
}
const known = new Set(KNOWN_RUST_CYCLES.map((c) => c.join(',')));
const found = cycles(rustGraph);
for (const c of found) {
  if (!known.has(c.join(','))) errors.push(`Rust : cycle entre modules ${c.join(' ↔ ')}`);
}
for (const k of known) {
  if (!found.some((c) => c.join(',') === k)) notes.push(`Rust : le cycle connu ${k} a disparu, retirez-le de KNOWN_RUST_CYCLES`);
}

// ---------- 2. Files shared with the installer ----------

const installerMain = readFileSync('src-installer/src/main.rs', 'utf8');
const shared = [...installerMain.matchAll(/#\[path = "\.\.\/\.\.\/src-tauri\/src\/([^"]+)"\]/g)].map((m) => m[1]);
// Module paths each shared file may use: the shared files themselves, as the
// installer re-creates them (src-installer/src/main.rs).
const sharedPaths = new Set(shared.map((f) => f.replace(/\.rs$/, '').replace(/\/mod$/, '').replaceAll('/', '::')));
for (const f of shared) {
  const code = rustCode(join(RUST, f));
  for (const m of code.matchAll(/\bcrate::((?:\w+::)*\w+)/g)) {
    const path = m[1];
    if (![...sharedPaths].some((p) => path === p || path.startsWith(`${p}::`)))
      errors.push(`Installateur : ${RUST}/${f} utilise crate::${path}, qui n'est pas compilé dans src-installer`);
  }
}

// ---------- 3. Frontend imports ----------

const SRC = 'src';
const tsFiles = [...walk(SRC)].filter((p) => /\.(ts|tsx)$/.test(p) && !/\.test\.tsx?$/.test(p) && !p.includes('/test/'));
const resolveImport = (from, spec) => {
  const base = normalize(join(dirname(from), spec));
  for (const cand of [base, `${base}.ts`, `${base}.tsx`, join(base, 'index.ts'), join(base, 'index.tsx')])
    if (existsSync(cand) && statSync(cand).isFile()) return cand;
  return null;
};
const tsGraph = new Map();
const tsAllImports = new Map();
for (const f of tsFiles) {
  const text = readFileSync(f, 'utf8');
  const runtime = new Set();
  const all = new Set();
  for (const m of text.matchAll(/^\s*(import|export)\s+(type\s+)?[^'"]*?from\s+['"](\.[^'"]+)['"]/gm)) {
    const target = resolveImport(f, m[3]);
    if (!target) continue;
    all.add(target);
    if (!m[2]) runtime.add(target);
  }
  for (const m of text.matchAll(/import\(['"](\.[^'"]+)['"]\)/g)) {
    const target = resolveImport(f, m[1]);
    if (target) all.add(target);
  }
  tsGraph.set(f, runtime);
  tsAllImports.set(f, all);
}
for (const c of cycles(tsGraph)) errors.push(`Frontend : cycle d'import ${c.join(' ↔ ')}`);
for (const [f, deps] of tsAllImports) {
  if (!f.startsWith(join(SRC, 'shared', 'ipc'))) continue;
  for (const d of deps)
    if (!d.startsWith(join(SRC, 'shared', 'ipc'))) errors.push(`Frontend : la couche IPC ${f} importe ${d} (l'IPC ne dépend pas de l'interface)`);
}

// ---------- 4. IPC contract ----------

const handler = (file) => {
  const m = readFileSync(file, 'utf8').match(/generate_handler!\[([\s\S]*?)\]/);
  return m ? m[1].split(',').map((s) => s.trim().split('::').pop()).filter(Boolean) : [];
};
const events = new Set();
for (const root of ['src-tauri/src', 'src-installer/src', 'src']) {
  for (const f of walk(root)) {
    if (!/\.(rs|ts|tsx)$/.test(f) || /\.test\.tsx?$/.test(f)) continue;
    const code = f.endsWith('.rs') ? rustCode(f) : readFileSync(f, 'utf8');
    for (const m of code.matchAll(/["']([a-z]+:\/\/[a-z-]+)["']/g)) events.add(m[1]);
  }
}
const contract = [
  '# Contrat IPC de Crosstalk, généré par scripts/check-architecture.mjs --write.',
  '# Toute différence fait échouer `npm run check:architecture` : renommer une',
  '# commande ou un événement casse la fenêtre qui l’utilise.',
  '',
  '[commandes app]',
  ...handler('src-tauri/src/lib.rs').sort(),
  '',
  '[commandes installateur]',
  ...handler('src-installer/src/main.rs').sort(),
  '',
  '[événements]',
  ...[...events].sort(),
  '',
].join('\n');
const CONTRACT = 'docs/architecture/ipc-contract.txt';
if (process.argv.includes('--write')) {
  writeFileSync(CONTRACT, contract);
  console.log(`écrit : ${CONTRACT}`);
} else if (!existsSync(CONTRACT) || readFileSync(CONTRACT, 'utf8') !== contract) {
  const before = existsSync(CONTRACT) ? new Set(readFileSync(CONTRACT, 'utf8').split('\n')) : new Set();
  const after = new Set(contract.split('\n'));
  const gone = [...before].filter((l) => l && !l.startsWith('#') && !after.has(l));
  const added = [...after].filter((l) => l && !l.startsWith('#') && !before.has(l));
  errors.push(
    `Contrat IPC modifié (${relative('.', CONTRACT)}) :` +
      gone.map((l) => `\n    - ${l}`).join('') +
      added.map((l) => `\n    + ${l}`).join('') +
      '\n  Si le changement est voulu : node scripts/check-architecture.mjs --write',
  );
}

notes.forEach((n) => console.log(`note : ${n}`));
if (errors.length) {
  console.error('ÉCHEC : règles d’architecture');
  errors.forEach((e) => console.error(`  ${e}`));
  process.exit(1);
}
console.log(
  `OK : ${modules.size} modules Rust, ${shared.length} fichiers partagés avec l'installateur, ${tsFiles.length} fichiers TypeScript, contrat IPC inchangé.`,
);
