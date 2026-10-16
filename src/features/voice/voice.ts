// "Voix IA": the Rust state mirrored for the card, and the helpers it shows.

import { create } from 'zustand';
import { isTauri, voice, VOICE_ENGINE_BYTES, type DownloadStatus, type PttPhase, type PttStatus, type SpeakState, type VoiceInfo, type VoiceOptions, type VoiceStatus } from '../../shared/ipc';

export interface VoiceState {
  native: boolean;
  status: VoiceStatus | null;
  download: DownloadStatus | null;
  /** Last sentence said in the game. */
  spoken: SpeakState | null;
  /** Automatic push-to-talk, live. */
  ptt: PttStatus | null;
}

export const useVoice = create<VoiceState>()(() => ({ native: isTauri, status: null, download: null, spoken: null, ptt: null }));

export async function refreshVoice(): Promise<void> {
  const status = await voice.status();
  if (status) useVoice.setState({ status, download: status.download });
}

/** Voices of a language, in catalogue order. */
export function voicesOf(all: VoiceInfo[], lang: string): VoiceInfo[] {
  return all.filter((v) => v.lang === lang);
}

/** The voice used for a language: the chosen one, else the first. */
export function chosenVoice(all: VoiceInfo[], opts: VoiceOptions, lang: string): VoiceInfo | null {
  const list = voicesOf(all, lang);
  return list.find((v) => v.id === opts.voices[lang]) ?? list[0] ?? null;
}

/** What is left to download for these languages: voice ids and bytes (engine included). */
export function toDownload(st: VoiceStatus, opts: VoiceOptions, langs: string[]): { ids: string[]; bytes: number } {
  const voices = langs.map((l) => chosenVoice(st.voices, opts, l)).filter((v): v is VoiceInfo => !!v && !v.installed);
  const bytes = voices.reduce((a, v) => a + v.onnxBytes + v.jsonBytes, 0) + (st.engineReady ? 0 : VOICE_ENGINE_BYTES);
  return { ids: voices.map((v) => v.id), bytes };
}

export const megabytes = (bytes: number) => `${Math.max(1, Math.round(bytes / 1_000_000))} Mo`;

export interface VoiceSummary {
  value: string;
  detail: string;
  color: string;
}

/** The state line of the card. */
export function voiceSummary(st: VoiceStatus | null, opts: VoiceOptions, ready: boolean): VoiceSummary {
  if (!opts.enabled) return { value: 'Désactivée', detail: 'Vos réponses dictées restent copiées pour le chat, comme avant.', color: '#6e6961' };
  if (!st) return { value: '…', detail: '', color: '#6e6961' };
  if (!st.cable) return { value: 'VB-CABLE introuvable', detail: 'Installez-le (étape 1), puis « Vérifier à nouveau ».', color: '#ff8a7a' };
  if (st.error) return { value: 'Problème', detail: st.error, color: '#ff8a7a' };
  if (!ready) return { value: 'Voix à télécharger', detail: 'Téléchargez le moteur et les voix (étape 3).', color: '#ffa24a' };
  if (st.speaking)
    return { value: 'La voix parle…', detail: opts.autoPtt ? 'Crosstalk maintient votre touche vocale pendant la phrase.' : 'Gardez votre touche vocale enfoncée si vous jouez en appui pour parler.', color: '#7fd4ff' };
  if (st.routing) return { value: 'Prête', detail: 'Mouse 5 / Ctrl+Alt+F6, parlez : la traduction est dite en jeu par la voix choisie.', color: '#7fe0a8' };
  return { value: 'En attente', detail: 'Le câble n’est pas encore alimenté.', color: '#ffa24a' };
}

/** Download progress in percent. */
export function downloadPercent(d: DownloadStatus | null): number | null {
  if (!d || d.state !== 'downloading' || !d.totalBytes) return null;
  return Math.min(100, Math.round((d.doneBytes / d.totalBytes) * 100));
}

/** The four steps shown while a sentence goes out with the voice key held. */
export const PTT_STEPS: [string, PttPhase[]][] = [
  ['Préparation voix', ['preparing', 'synthesized']],
  ['Activation micro', ['held']],
  ['Transmission', ['playing', 'releasing']],
  ['Terminé', ['done']],
];

/** Index of the current step (-1: nothing running), and the line under it. */
export function pttView(s: PttStatus | null): { step: number; text: string; color: string; busy: boolean } {
  if (!s || s.phase === 'idle') return { step: -1, text: '', color: '#6e6961', busy: false };
  if (s.phase === 'error') return { step: -1, text: `Erreur : ${s.detail ?? 'inconnue'}`, color: '#ff8a7a', busy: false };
  if (s.phase === 'cancelled') return { step: -1, text: 'Transmission annulée, touche relâchée.', color: '#ffa24a', busy: false };
  const step = PTT_STEPS.findIndex(([, phases]) => phases.includes(s.phase));
  const busy = s.phase !== 'done';
  if (s.detail) return { step, text: s.detail, color: '#ffa24a', busy };
  if (s.phase === 'done') return { step, text: s.pressed ? 'Phrase transmise, touche relâchée.' : 'Phrase jouée.', color: '#7fe0a8', busy };
  return { step, text: '', color: '#7fd4ff', busy };
}

/** A key name Crosstalk can press (same list as src-tauri/src/voice/ptt.rs). */
export function validPttKey(name: string): boolean {
  const n = name.trim().toUpperCase();
  if (/^[A-Z0-9]$/.test(n)) return true;
  if (/^F([1-9]|1[0-2])$/.test(n)) return true;
  return ['SPACE', 'TAB', 'CAPSLOCK', 'CAPS', 'SHIFT', 'LSHIFT', 'RSHIFT', 'CTRL', 'LCTRL', 'RCTRL', 'ALT', 'LALT', 'RALT', 'MOUSE3', 'MOUSE4', 'MOUSE5'].includes(n);
}
