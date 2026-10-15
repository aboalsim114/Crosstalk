// "Voix IA": the Rust state mirrored for the card, and the helpers it shows.

import { create } from 'zustand';
import { isTauri, voice, VOICE_ENGINE_BYTES, type DownloadStatus, type SpeakState, type VoiceInfo, type VoiceOptions, type VoiceStatus } from '../../shared/ipc';

export interface VoiceState {
  native: boolean;
  status: VoiceStatus | null;
  download: DownloadStatus | null;
  /** Last sentence said in the game. */
  spoken: SpeakState | null;
}

export const useVoice = create<VoiceState>()(() => ({ native: isTauri, status: null, download: null, spoken: null }));

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
  if (st.speaking) return { value: 'La voix parle…', detail: 'Gardez votre touche vocale enfoncée si vous jouez en appui pour parler.', color: '#7fd4ff' };
  if (st.routing) return { value: 'Prête', detail: 'Ctrl+Alt+F6, parlez : la traduction est dite en jeu par la voix choisie.', color: '#7fe0a8' };
  return { value: 'En attente', detail: 'Le câble n’est pas encore alimenté.', color: '#ffa24a' };
}

/** Download progress in percent. */
export function downloadPercent(d: DownloadStatus | null): number | null {
  if (!d || d.state !== 'downloading' || !d.totalBytes) return null;
  return Math.min(100, Math.round((d.doneBytes / d.totalBytes) * 100));
}
