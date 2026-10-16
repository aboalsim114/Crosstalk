// What's new for the players, from release-notes.json (one entry per change
// they can notice, written in plain words with its pull request).
//
//   node scripts/release-notes.mjs <build> '<previous news.json>'
//
// Prints {"notes": [...], "news": [...]}: each entry keeps the build that first
// shipped it (from the previous news.json, else the build being published),
// newest first. `notes` (sentences of the newest entry) is what older versions
// of the app show. Run with --check to validate the file only.
import { readFileSync } from 'node:fs';

/** Words that belong to developers, not to players (whole words, accents count as letters). */
const word = (w) => `(?<![\\p{L}\\d])(?:${w})(?![\\p{L}\\d])`;
export const DEV_WORDS = new RegExp(
  ['feat', 'fix', 'chore', 'refactor', 'commit', 'github', 'sha-?256', 'pull request', 'merge', 'ipc', 'whisper-rs'].map(word).join('|') + '|#\\d+|\\.(?:rs|tsx?|mjs)(?![\\p{L}\\d])',
  'iu',
);
/** Case-sensitive: "PR" the abbreviation, not "pr" inside a word. */
const PR = new RegExp(word('PR'), 'u');

export function validate(entries) {
  const errors = [];
  if (!Array.isArray(entries)) return ['la liste des notes doit être un tableau'];
  const ids = new Set();
  entries.forEach((e, i) => {
    const at = `entrée ${i + 1}${e?.id ? ` (${e.id})` : ''}`;
    if (typeof e?.id !== 'string' || !/^[a-z0-9-]{3,60}$/.test(e.id)) errors.push(`${at} : id manquant ou invalide`);
    else if (ids.has(e.id)) errors.push(`${at} : id en double`);
    else ids.add(e.id);
    if (e.build != null && !(Number.isInteger(e.build) && e.build > 0)) errors.push(`${at} : build invalide`);
    if (typeof e?.date !== 'string' || !/^\d{4}-\d{2}-\d{2}$/.test(e.date)) errors.push(`${at} : date AAAA-MM-JJ attendue`);
    if (typeof e?.title !== 'string' || e.title.length < 3 || e.title.length > 80) errors.push(`${at} : titre de 3 à 80 caractères`);
    if (!Array.isArray(e?.items) || e.items.length === 0 || e.items.length > 6) errors.push(`${at} : 1 à 6 phrases`);
    for (const text of [e?.title, ...(Array.isArray(e?.items) ? e.items : [])]) {
      if (typeof text !== 'string' || text.length > 240) errors.push(`${at} : phrase vide ou trop longue`);
      else if (DEV_WORDS.test(text) || PR.test(text)) errors.push(`${at} : vocabulaire de développeur dans « ${text} »`);
    }
  });
  return errors;
}

export function assign(entries, build, previous) {
  const known = new Map((Array.isArray(previous) ? previous : []).filter((p) => p?.id && Number.isInteger(p.build)).map((p) => [p.id, p.build]));
  const news = entries
    .map((e) => ({ id: e.id, build: e.build ?? known.get(e.id) ?? build, date: e.date, title: e.title, items: e.items }))
    .sort((a, b) => b.build - a.build);
  const newest = news[0];
  return { notes: newest ? newest.items.slice(0, 4) : [], news };
}

if (import.meta.url === `file://${process.argv[1]}`) {
  const entries = JSON.parse(readFileSync(new URL('../release-notes.json', import.meta.url), 'utf8'));
  const errors = validate(entries);
  if (errors.length) {
    console.error(`ÉCHEC : release-notes.json\n  ${errors.join('\n  ')}`);
    process.exit(1);
  }
  if (process.argv[2] === '--check') {
    console.log(`OK : ${entries.length} note(s) de version, sans vocabulaire de développeur.`);
  } else {
    const build = Number(process.argv[2]);
    let previous = [];
    try {
      previous = JSON.parse(process.argv[3] || '[]');
    } catch {
      previous = [];
    }
    process.stdout.write(JSON.stringify(assign(entries, build, previous)));
  }
}
