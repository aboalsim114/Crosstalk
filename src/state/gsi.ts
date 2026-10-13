// Map being played, from CS2's Game State Integration (src-tauri/src/gsi).

import { create } from 'zustand';
import { gsi, isTauri, type GsiStatus } from '../backend';
import { useChat, type Summary } from './chat';

export const useGsi = create<{ status: GsiStatus | null }>()(() => ({ status: null }));

export function startGsiBridge(): () => void {
  if (!isTauri) return () => {};
  gsi.status().then((status) => status && useGsi.setState({ status }));
  const un = gsi.onStatus((status) => useGsi.setState({ status }));
  // CS2's folder can change (manual path): look for the cfg file again.
  const unChat = useChat.subscribe((s, prev) => {
    if (s.status?.setup.cs2Dir !== prev.status?.setup.cs2Dir) gsi.status().then((status) => status && useGsi.setState({ status }));
  });
  return () => {
    unChat();
    void un.then((f) => f());
  };
}

/** What the map card says. */
export function gsiSummary(st: GsiStatus | null): Summary {
  if (!st) return { value: 'Recherche de CS2…', detail: '', color: '#6e6961' };
  if (!st.cs2Found) return { value: 'CS2 introuvable', detail: 'Indiquez le dossier de CS2 ci-dessus.', color: '#ff8a7a' };
  if (!st.installed) return { value: 'Désactivée', detail: 'Les annonces sont reconnues avec le vocabulaire CS général.', color: '#6e6961' };
  if (st.error) return { value: 'Indisponible', detail: st.error, color: '#ff8a7a' };
  if (st.map) {
    const voc = st.callouts ? `${st.callouts} callouts de ${st.mapName} utilisés pour la reconnaissance et conservés dans la traduction.` : 'Carte sans liste de callouts : vocabulaire CS général.';
    return { value: `Carte : ${st.mapName ?? st.map}`, detail: voc, color: '#7fe0a8' };
  }
  if (st.lastSeen) return { value: 'CS2 connecté · dans les menus', detail: 'La carte s’affichera au chargement d’une partie.', color: '#ffa24a' };
  return { value: 'Activée · en attente de CS2', detail: 'Relancez CS2 s’il était déjà ouvert : il lit ce réglage au démarrage.', color: '#ffa24a' };
}
