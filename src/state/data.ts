// Static content and simulated events, ported 1:1 from the Claude Design prototype
// (project/CS2 Live Translator v2 Cinematic.dc.html).

export type Lang = 'ru' | 'en' | 'fr';
export type Channel = 'CT' | 'T' | 'ALL';

export interface SimEvent {
  kind: 'voice' | 'chat';
  src: Lang;
  orig: string;
  /** Provisional (streaming) transcription shown before the final one. Voice only. */
  prov?: string;
  /** Final translation; null when the line is already in French. */
  tr: string | null;
  ch?: Channel;
  sender?: string;
}

export const EVENTS: SimEvent[] = [
  { kind: 'voice', src: 'ru', orig: 'Двое идут на B, двое на B', prov: 'Deux joueurs arrivent sur…', tr: 'Deux joueurs arrivent sur B, deux sur B.' },
  { kind: 'chat', ch: 'CT', sender: 'Player123', src: 'en', orig: 'gonna smoke mid, wait for my flash', tr: 'Je vais poser une smoke au mid, attendez ma flash.' },
  { kind: 'voice', src: 'en', orig: 'one low, he is lit, eighty HP', prov: 'Un en bas, il est…', tr: 'Un en bas, il est touché, 80 PV.' },
  { kind: 'chat', ch: 'ALL', sender: 'Дима', src: 'ru', orig: 'гг, хорошая игра', tr: 'gg, bonne partie.' },
  { kind: 'voice', src: 'ru', orig: 'Сейвим, не покупайте, эко раунд', prov: 'On save, n’achetez…', tr: 'On save, n’achetez rien, round eco.' },
  { kind: 'chat', ch: 'T', sender: 'mira: the ghost', src: 'en', orig: 'drop AWP pls', tr: 'Drop l’AWP stp.' },
  { kind: 'voice', src: 'ru', orig: 'Бомба на A, ротейт быстро', prov: 'Bombe sur A…', tr: 'Bombe sur A, rotate vite.' },
  { kind: 'chat', ch: 'CT', sender: 'Lucas', src: 'fr', orig: 'je prends B avec toi', tr: null },
];

export const LANG_NAME: Record<Lang, string> = { ru: 'RUSSE', en: 'ANGLAIS', fr: 'FRANÇAIS' };
export const CHANNEL_NAME: Record<Channel, string> = { CT: 'ÉQUIPE CT', T: 'ÉQUIPE T', ALL: 'TOUS' };

export type PageId = 'dash' | 'overlay' | 'audio' | 'lang' | 'hist' | 'diag' | 'about';
export const NAV: [PageId, string][] = [
  ['dash', 'Accueil'],
  ['overlay', 'Overlay'],
  ['audio', 'Audio & IA'],
  ['lang', 'Langues & glossaire'],
  ['hist', 'Historique'],
  ['diag', 'Diagnostic'],
  ['about', 'À propos'],
];

export type ModelId = 'tiny' | 'base' | 'small' | 'medium' | 'large-v3-turbo';
/** [id, size, description, installed by default] */
export const MODELS: [ModelId, string, string, boolean][] = [
  ['tiny', '75 Mo', 'Très rapide · précision faible', true],
  ['base', '142 Mo', 'Rapide · CPU seul possible', true],
  ['small', '466 Mo', 'Équilibré — recommandé', true],
  ['medium', '1,5 Go', 'Précis · GPU conseillé', false],
  ['large-v3-turbo', '1,6 Go', 'Le plus précis · GPU requis', false],
];

export type GlossMode = 'Conserver' | 'Traduire';
export type GlossEntry = [term: string, mode: GlossMode, rendering: string];
export const GLOSSARY: GlossEntry[] = [
  ['rush B', 'Conserver', 'rush B'],
  ['mid', 'Conserver', 'mid'],
  ['smoke', 'Conserver', 'smoke'],
  ['flash', 'Conserver', 'flash'],
  ['eco', 'Conserver', 'eco'],
  ['plant', 'Conserver', 'plant'],
  ['defuse', 'Conserver', 'defuse'],
  ['AWP', 'Conserver', 'AWP'],
  ['rotate', 'Conserver', 'rotate'],
  ['save', 'Conserver', 'save'],
  ['CT / T', 'Conserver', 'CT / T'],
  ['one low', 'Traduire', 'un en bas (PV faibles)'],
  ['lit', 'Traduire', 'touché'],
];

/** Original → translation pairs orbiting the 3D headset. */
export const ORBIT_PAIRS: [string, string][] = [
  ['Двое на B', 'Deux sur B'],
  ['rush B', 'rush B'],
  ['one low', 'un en bas'],
  ['эко раунд', 'round eco'],
  ['smoke mid', 'smoke au mid'],
  ['ротейт', 'rotate'],
  ['he is lit', 'il est touché'],
  ['бомба на A', 'bombe sur A'],
];

export const NEVERS = [
  'Injecter une DLL dans cs2.exe',
  'Lire ou écrire la mémoire du jeu',
  'Intercepter ou modifier les paquets réseau',
  'Contourner VAC ou le mode approuvé',
  'Écrire dans le chat sans action de votre part',
];

export const LICENSES: [string, string][] = [
  ['tauri', 'MIT / Apache-2.0'],
  ['whisper.cpp', 'MIT'],
  ['whisper-rs', 'Unlicense'],
  ['tokio', 'MIT'],
  ['windows-rs', 'MIT / Apache-2.0'],
  ['rubato', 'MIT'],
  ['three.js (rendu 3D)', 'MIT'],
];

export type OverlayLine = Pick<SimEvent, 'kind' | 'src' | 'orig' | 'ch' | 'sender'> & { shown: string | null };

/** Placeholder lines for the overlay preview before any live line exists. */
export const SAMPLE_OVERLAY: OverlayLine[] = [
  { kind: 'voice', src: 'ru', orig: 'Двое идут на B', shown: 'Deux joueurs arrivent sur B.' },
  { kind: 'chat', ch: 'CT', src: 'en', sender: 'Player123', orig: 'gonna smoke mid', shown: 'Je vais poser une smoke au mid.' },
  { kind: 'voice', src: 'en', orig: 'one low, he is lit', shown: 'Un en bas, il est touché.' },
];
