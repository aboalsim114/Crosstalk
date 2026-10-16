// Static content and simulated events, ported 1:1 from the Claude Design prototype
// (project/CS2 Live Translator v2 Cinematic.dc.html).

import type { PlayerLang } from '../../features/languages/langs';

/** 'xx': written chat whose language could not be told (shown as is). */
export type Lang = PlayerLang | 'xx';
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

/** Speech recognition model (src-tauri/src/speech/models.rs). */
export type ModelId = 'parakeet-tdt-0.6b-v3';
export const DEFAULT_MODEL: ModelId = 'parakeet-tdt-0.6b-v3';
/** [id, label, size, description] */
export const MODELS: [ModelId, string, string, string][] = [['parakeet-tdt-0.6b-v3', 'Parakeet TDT 0.6B v3', '670 Mo', 'Russe, anglais, français et 22 autres langues · processeur']];

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
  ['CT', 'Conserver', 'CT'],
  ['T', 'Conserver', 'T'],
  ['one low', 'Traduire', 'un ennemi à peu de PV'],
  ['lit', 'Traduire', 'touché (gros dégâts)'],
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

/**
 * What the shipped app is built from (src-tauri/Cargo.toml, package.json) and
 * the models it downloads, with their licences as published (Hugging Face
 * metadata checked on 2026-10-14). Keep in step with the dependencies.
 */
export const LICENSES: [string, string][] = [
  ['tauri', 'MIT / Apache-2.0'],
  ['sherpa-onnx', 'Apache-2.0'],
  ['ONNX Runtime', 'MIT'],
  ['windows-rs', 'MIT / Apache-2.0'],
  ['ureq', 'MIT / Apache-2.0'],
  ['React · three.js · motion', 'MIT'],
  ['Parakeet TDT 0.6B v3 (NVIDIA)', 'CC-BY-4.0'],
  ['Silero VAD', 'MIT'],
];

/** `status: 'transcrit'` marks a real transcript shown in its original language. */
export type OverlayLine = Pick<SimEvent, 'kind' | 'src' | 'orig' | 'ch' | 'sender'> & { shown: string | null; status?: string; live?: boolean };

/** Placeholder lines for the overlay preview before any live line exists. */
export const SAMPLE_OVERLAY: OverlayLine[] = [
  { kind: 'voice', src: 'ru', orig: 'Двое идут на B', shown: 'Deux joueurs arrivent sur B.' },
  { kind: 'chat', ch: 'CT', src: 'en', sender: 'Player123', orig: 'gonna smoke mid', shown: 'Je vais poser une smoke au mid.' },
  { kind: 'voice', src: 'en', orig: 'one low, he is lit', shown: 'Un en bas, il est touché.' },
];
