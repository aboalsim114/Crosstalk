// In-app update: the Rust updater reads the update feed, downloads and
// verifies the newer installer, then runs it (src-tauri/src/updater.rs).
// Everything shown here is written for players: no build hash, no
// repository, no checksum.

import { create } from 'zustand';
import { isTauri, updater, type NewsEntry, type UpdateStatus } from '../../shared/ipc';

export interface UpdateStore {
  native: boolean;
  status: UpdateStatus | null;
  /** The banner was closed for this offered build. */
  dismissed: number | null;
}

export const useUpdate = create<UpdateStore>()(() => ({ native: isTauri, status: null, dismissed: null }));

const mb = (b: number) => `${Math.round(b / 1e6)} Mo`;

const MONTHS = ['janvier', 'février', 'mars', 'avril', 'mai', 'juin', 'juillet', 'août', 'septembre', 'octobre', 'novembre', 'décembre'];

/** "2026-10-16" or "2026-10-16 01:35 UTC" → "16 octobre 2026". */
export function formatDate(date: string | undefined): string {
  const m = /^(\d{4})-(\d{2})-(\d{2})/.exec(date ?? '');
  if (!m || Number(m[2]) < 1 || Number(m[2]) > 12) return date ?? '';
  return `${Number(m[3])} ${MONTHS[Number(m[2]) - 1]} ${m[1]}`;
}

/** The version as players see it: "0.4.1.276". */
export function versionLabel(version: string, build: number | undefined): string {
  return build ? `${version}.${build}` : version;
}

const news = (st: UpdateStatus | null): NewsEntry[] => st?.latest?.news ?? [];

/** What the offered update brings (newer than the installed version). */
export function upcomingNews(st: UpdateStatus | null): NewsEntry[] {
  return st ? news(st).filter((n) => n.build > st.build) : [];
}

/** What the installed version brought, newest first. */
export function installedNews(st: UpdateStatus | null, max = 3): NewsEntry[] {
  return st ? news(st).filter((n) => n.build <= st.build).slice(0, max) : [];
}

/** What changed since the version the player last saw (shown once after an update). */
export function newsSince(st: UpdateStatus | null, lastSeen: number): NewsEntry[] {
  return st ? news(st).filter((n) => n.build > lastSeen && n.build <= st.build) : [];
}

/** The updater's technical errors, said in plain words (the detail stays in the journal). */
export function friendlyError(raw: string | null | undefined): string {
  const e = raw ?? '';
  if (/annulé/.test(e)) return 'Téléchargement annulé.';
  if (/interrompu/.test(e)) return 'Téléchargement interrompu. Vérifiez votre connexion, puis réessayez.';
  if (/SHA-256|taille|plus grand|empreinte/i.test(e)) return 'Le fichier téléchargé était incomplet : il a été supprimé. Réessayez.';
  if (/écriture|installation impossible/.test(e)) return 'Impossible d’enregistrer la mise à jour sur ce PC. Vérifiez l’espace disque, puis réessayez.';
  if (/lancement de l'installateur/.test(e)) return 'Impossible de lancer l’installation. Redémarrez Crosstalk, puis réessayez.';
  if (/recherche impossible|illisible|refusée|404|connexion/i.test(e)) return UNAVAILABLE;
  return 'Mise à jour impossible pour le moment. Réessayez plus tard.';
}

export const UNAVAILABLE = 'Impossible de vérifier les mises à jour pour le moment. Vérifiez votre connexion, puis réessayez plus tard.';

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
  switch (st.state) {
    case 'available': {
      const titles = upcomingNews(st).map((n) => n.title);
      return {
        ...none,
        banner: dismissed !== l?.build,
        title: 'Nouvelle version disponible',
        detail: titles.length ? titles.slice(0, 2).join(' · ') : `Publiée le ${formatDate(l?.date)}`,
        action: 'download',
      };
    }
    case 'downloading': {
      const pct = st.totalBytes ? Math.min(100, Math.round((st.doneBytes / st.totalBytes) * 100)) : 0;
      return { ...none, banner: true, title: `Téléchargement de la mise à jour · ${pct} %`, detail: `${mb(st.doneBytes)} / ${mb(st.totalBytes)} · vérifiée avant l’installation`, percent: pct };
    }
    case 'ready':
      return {
        ...none,
        banner: true,
        title: 'Mise à jour prête',
        detail: sessionRunning ? 'Arrêtez la session pour installer : Crosstalk redémarre en quelques secondes.' : 'Crosstalk va se fermer, s’installer et redémarrer. Vos réglages sont conservés.',
        action: 'install',
        blocked: sessionRunning,
      };
    case 'installing':
      return { ...none, banner: true, title: 'Installation de la mise à jour…', detail: 'Crosstalk redémarre dans quelques secondes.' };
    case 'error':
      return { ...none, banner: l != null && l.build > st.build && dismissed !== l.build, title: 'Mise à jour impossible', detail: friendlyError(st.error), action: 'retry' };
    default:
      return none;
  }
}

/** One line for the About page. */
export function updateSummary(st: UpdateStatus | null): string {
  if (!st) return 'Les mises à jour se font depuis l’application Windows.';
  switch (st.state) {
    case 'checking':
      return 'Recherche d’une mise à jour…';
    case 'uptodate':
      return 'Crosstalk est à jour.';
    case 'available':
      return `Une nouvelle version est disponible (${formatDate(st.latest?.date)}).`;
    case 'downloading':
      return 'Téléchargement…';
    case 'ready':
      return 'Mise à jour téléchargée, prête à installer.';
    case 'installing':
      return 'Installation…';
    case 'error':
      return friendlyError(st.error);
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
