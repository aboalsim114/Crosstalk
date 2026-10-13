// "Qui parle ?" (experimental, off by default): the Rust core reads the
// names on CS2's voice indicator with Windows' OCR and puts the speaker's
// name on voice lines (src-tauri/src/speaker). Names stay in memory.

import { create } from 'zustand';
import { isTauri, speakerId, type SpeakerConfig, type SpeakerStatus } from '../backend';
import { useApp, type AppState } from './store';

export const useSpeakerStatus = create<{ status: SpeakerStatus | null }>()(() => ({ status: null }));

/** Rust options: the user's own name is never credited (CS2 lists it too). */
export function speakerConfigFrom(s: Pick<AppState, 'speaker' | 'ownName'>, extraIgnore: string[] = []): SpeakerConfig {
  const own = s.ownName.trim();
  return {
    enabled: s.speaker.enabled,
    region: s.speaker.region,
    ignore: [...(own ? [own] : []), ...extraIgnore],
  };
}

export function speakerSummary(st: SpeakerStatus | null, enabled: boolean): { value: string; detail: string; color: string } {
  if (!enabled)
    return {
      value: 'Désactivé',
      detail: 'L’écran n’est pas lu.',
      color: '#6e6961',
    };
  if (!st || st.state === 'off') return { value: 'Démarrage…', detail: '', color: '#6e6961' };
  switch (st.state) {
    case 'error':
      return {
        value: 'Indisponible',
        detail: st.detail ?? 'Erreur inconnue',
        color: '#ff8a7a',
      };
    case 'waiting':
      return {
        value: 'En attente de CS2',
        detail: st.detail ?? '',
        color: '#ffa24a',
      };
    case 'paused':
      return {
        value: 'En pause',
        detail: 'CS2 est en arrière-plan : l’écran n’est lu que quand CS2 est au premier plan.',
        color: '#ffa24a',
      };
    case 'watching':
      if (st.blank)
        return {
          value: 'Écran noir',
          detail: 'CS2 en plein écran exclusif ne peut pas être lu : passez en « plein écran fenêtré » (Paramètres vidéo › Mode d’affichage).',
          color: '#ff8a7a',
        };
      if (st.covered)
        return {
          value: 'Zone recouverte',
          detail: 'L’overlay Crosstalk recouvre la zone lue : déplacez-le dans un autre coin.',
          color: '#ffa24a',
        };
      return {
        value: 'Actif',
        detail: st.known.length ? `${st.known.length} pseudo${st.known.length > 1 ? 's' : ''} lu${st.known.length > 1 ? 's' : ''} pendant cette session.` : 'Lecture quand quelqu’un parle. Aucun pseudo lu pour l’instant.',
        color: '#7fe0a8',
      };
  }
  return { value: '…', detail: '', color: '#6e6961' };
}

/** Keeps the Rust options in sync and mirrors the status. */
export function startSpeakerBridge(): () => void {
  if (!isTauri) return () => {};
  const push = () => void speakerId.configure(speakerConfigFrom(useApp.getState()));
  push();
  void speakerId.status().then((status) => useSpeakerStatus.setState({ status }));
  const stopStore = useApp.subscribe((s, prev) => {
    if (s.speaker !== prev.speaker || s.ownName !== prev.ownName) push();
  });
  const unlisten = speakerId.onStatus((status) => useSpeakerStatus.setState({ status }));
  return () => {
    stopStore();
    void unlisten.then((f) => f());
  };
}
