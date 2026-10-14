// Non-negotiable constraint (docs/cahier-des-charges.md §3): Crosstalk never
// opens, reads, writes or hooks another process. Comment lines are ignored.
import { readdirSync, readFileSync, statSync } from 'node:fs';
import { join } from 'node:path';

const ROOTS = ['src-tauri/src', 'src-tauri/examples', 'src-installer/src', 'src-llm/src'];
const FORBIDDEN =
  /\b(OpenProcess|ReadProcessMemory|WriteProcessMemory|NtReadVirtualMemory|NtWriteVirtualMemory|VirtualAllocEx|CreateRemoteThread|SetWindowsHookEx[AW]?|DebugActiveProcess)\b/;

function* files(dir) {
  for (const name of readdirSync(dir)) {
    const path = join(dir, name);
    if (statSync(path).isDirectory()) yield* files(path);
    else yield path;
  }
}

const hits = [];
for (const root of ROOTS) {
  for (const path of files(root)) {
    readFileSync(path, 'utf8')
      .split('\n')
      .forEach((line, i) => {
        if (FORBIDDEN.test(line) && !/^\s*\/\//.test(line)) hits.push(`${path}:${i + 1}: ${line.trim()}`);
      });
  }
}

if (hits.length) {
  console.error('ÉCHEC : API d’accès à un autre processus interdite :');
  hits.forEach((h) => console.error(`  ${h}`));
  process.exit(1);
}
console.log('OK : aucune API d’accès aux processus dans les sources Rust.');
