// In-app guide (SAB-20, V1): content model, local search and navigation state.
// Everything is bundled with the app: the guide works offline.

import { create } from 'zustand';
import type { PageId } from '../../app/state/data';

/** How far a feature can be trusted today. */
export type FeatureStatus = 'disponible' | 'à configurer' | 'expérimental';

export type CategoryId = 'start' | 'match' | 'more' | 'help';

export const CATEGORIES: [CategoryId, string][] = [
  ['start', 'Bien démarrer'],
  ['match', 'En partie'],
  ['more', 'Fonctions avancées'],
  ['help', 'Dépannage'],
];

/** A screenshot of a real screen, with numbered notes placed over it (x, y in % of the image). */
export interface Shot {
  src: string;
  alt: string;
  notes: { x: number; y: number; text: string }[];
}

export interface Chapter {
  id: string;
  category: CategoryId;
  title: string;
  /** One line under the title. */
  summary: string;
  status: FeatureStatus;
  /** Why it is not "disponible", or what is left to check. */
  statusNote?: string;
  /** "Ouvrir les paramètres": the screen where it is set. */
  settings?: { page: PageId; label: string };
  purpose: string;
  prerequisites: string[];
  setup: string[];
  use: string[];
  check: string[];
  limits: string[];
  shot?: Shot;
}

export interface Faq {
  q: string;
  a: string;
  /** Chapter to read for more. */
  chapter: string;
}

/** Lower case, no accents, single spaces: "Écran " and "ecran" match. */
export const normalize = (s: string) =>
  s
    .toLowerCase()
    .normalize('NFD')
    .replace(/[̀-ͯ]/g, '')
    .replace(/[^\p{L}\p{N}]+/gu, ' ')
    .trim();

export type Hit = { kind: 'chapter'; chapter: Chapter; score: number; excerpt: string } | { kind: 'faq'; faq: Faq; score: number; excerpt: string };

function chapterText(c: Chapter): string[] {
  return [c.summary, c.purpose, ...c.prerequisites, ...c.setup, ...c.use, ...c.check, ...c.limits, ...(c.shot?.notes.map((n) => n.text) ?? [])];
}

/**
 * Chapters and FAQ entries containing every word of `query` (accents and case
 * ignored), title matches first. The excerpt is the first passage that matches.
 */
export function search(query: string, chapters: Chapter[], faqs: Faq[]): Hit[] {
  const words = normalize(query).split(' ').filter((w) => w.length > 1);
  if (!words.length) return [];
  const all = (text: string) => {
    const n = normalize(text);
    return words.every((w) => n.includes(w));
  };
  const any = (text: string) => {
    const n = normalize(text);
    return words.some((w) => n.includes(w));
  };
  const hits: Hit[] = [];
  for (const c of chapters) {
    const texts = chapterText(c);
    if (!all([c.title, ...texts].join(' '))) continue;
    const score = (all(c.title) ? 100 : any(c.title) ? 50 : 0) + texts.filter(any).length;
    hits.push({ kind: 'chapter', chapter: c, score, excerpt: texts.find(all) ?? texts.find(any) ?? c.summary });
  }
  for (const f of faqs) {
    if (!all(`${f.q} ${f.a}`)) continue;
    hits.push({ kind: 'faq', faq: f, score: (all(f.q) ? 40 : 10) + (any(f.a) ? 1 : 0), excerpt: f.a });
  }
  return hits.sort((a, b) => b.score - a.score);
}

/** The chapter that explains a screen (the "?" of each page header). */
export const PAGE_CHAPTER: Partial<Record<PageId, string>> = {
  dash: 'first-launch',
  overlay: 'overlay',
  audio: 'audio',
  lang: 'languages',
  hist: 'diagnostics',
  diag: 'diagnostics',
  about: 'first-launch',
};

interface GuideState {
  /** Chapter open, null for the table of contents (or 'faq'). */
  chapter: string | null;
  query: string;
  /** Screen to go back to when the guide is closed. */
  returnTo: PageId;
  set: (patch: Partial<Pick<GuideState, 'chapter' | 'query' | 'returnTo'>>) => void;
}

export const useGuide = create<GuideState>((set) => ({
  chapter: null,
  query: '',
  returnTo: 'dash',
  set: (patch) => set(patch),
}));
