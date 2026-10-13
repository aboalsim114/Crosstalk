// In-app update: the Rust updater reads the update feed, downloads and
// verifies the newer installer, then runs it (src-tauri/src/updater.rs).

import { create } from 'zustand';
import { isTauri, updater, type UpdateStatus } from '../backend';

export interface UpdateStore {
  native: boolean;
  status: UpdateStatus | null;
  /** The banner was closed for this offered build. */
  dismissed: number | null;
}

export const useUpdate = create<UpdateStore>()(() => ({ native: isTauri, status: null, dismissed: null }));

const mb = (b: number) => `${Math.round(b / 1e6)} Mo`;

export interface UpdateView {
  /** Shown in the banner on every page. */
  banner: boolean;
  title: string;
  detail: string;
  percent: number | null;
  action: 'download' | 'install' | 'retry' | null;
  /** Install waits for the end of the session. */
  blocked: boolean;
}

export function updateView(st: UpdateStatus | null, dismissed: number | null, sessionRunning: boolean): UpdateView {
  const none: UpdateView = { banner: false, title: '', detail: '', percent: null, action: null, blocked: false };
  if (!st) return none;
  const l = st.latest;
  const notes = l?.notes.length ? l.notes.slice(0, 2).join(' · ') : '';
  switch (st.state) {
    case 'available':
      return { ...none, banner: dismissed !== l?.build, title: `Nouvelle version disponible · build ${l?.build}`, detail: notes || `Publiée le ${l?.date}`, action: 'download' };
    case 'downloading': {
      const pct = st.totalBytes ? Math.min(100, Math.round((st.doneBytes / st.totalBytes) * 100)) : 0;
      return { ...none, banner: true, title: `Téléchargement de la mise à jour · ${pct} %`, detail: `${mb(st.doneBytes)} / ${mb(st.totalBytes)} · fichier vérifié (taille, SHA-256) avant installation`, percent: pct };
    }
    case 'ready':
      return {
        ...none,
        banner: true,
        title: 'Mise à jour prête',
        detail: sessionRunning ? 'Arrêtez la session pour installer : Crosstalk redémarre en quelques secondes.' : 'Crosstalk va se fermer, s’installer et redémarrer. Modèles et réglages conservés.',
        action: 'install',
        blocked: sessionRunning,
      };
    case 'installing':
      return { ...none, banner: true, title: 'Installation de la mise à jour…', detail: 'Crosstalk redémarre dans quelques secondes.' };
    case 'error':
      return { ...none, banner: l != null && l.build > st.build && dismissed !== l.build, title: 'Mise à jour impossible', detail: st.error ?? 'erreur', action: 'retry' };
    default:
      return none;
  }
}

export const PRIVATE_FEED =
  'Mises à jour indisponibles : le dépôt de publication est privé. Elles fonctionneront dès qu’un emplacement public sera choisi ; en attendant, réinstallez depuis le lien du README.';

/** One line for the About page. */
export function updateSummary(st: UpdateStatus | null): string {
  if (!st) return 'Mise à jour depuis l’application disponible dans la version Windows.';
  switch (st.state) {
    case 'checking':
      return 'Recherche…';
    case 'uptodate':
      return 'Crosstalk est à jour.';
    case 'available':
      return `Build ${st.latest?.build} disponible (${st.latest?.date}).`;
    case 'downloading':
      return 'Téléchargement…';
    case 'ready':
      return 'Mise à jour téléchargée et vérifiée.';
    case 'installing':
      return 'Installation…';
    case 'error':
      // GitHub answers 404 to anyone outside a private repository.
      if (st.error?.includes('404') && !st.latest) return PRIVATE_FEED;
      return st.error ?? 'Erreur';
    default:
      return 'Pas encore vérifié.';
  }
}

/** Checks a few seconds after launch, then every 6 hours. */
export function startUpdateBridge(): () => void {
  if (!isTauri) return () => {};
  updater.status().then((status) => status && useUpdate.setState({ status }));
  const un = updater.onStatus((status) => useUpdate.setState({ status }));
  const first = setTimeout(() => updater.check(), 8000);
  const every = setInterval(() => {
    const s = useUpdate.getState().status?.state;
    if (s !== 'downloading' && s !== 'ready' && s !== 'installing') updater.check();
  }, 6 * 3600 * 1000);
  return () => {
    clearTimeout(first);
    clearInterval(every);
    un.then((f) => f());
  };
}
