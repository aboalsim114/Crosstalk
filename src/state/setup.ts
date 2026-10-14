// First-run setup: the app downloads its models itself (installer stays small).

import { create } from 'zustand';
import { modelDownload, type DownloadStatus } from '../backend';
import type { AppState } from './store';

export const useSetup = create<{ status: DownloadStatus | null }>()(() => ({ status: null }));

export const mb = (bytes: number) => `${Math.round(bytes / 1_000_000)} Mo`;

export interface SetupView {
  visible: boolean;
  title: string;
  detail: string;
  percent: number | null;
  tone: 'progress' | 'error' | 'done';
  canCancel: boolean;
  canRetry: boolean;
}

/** What the setup banner shows; hidden once every model is installed. */
export function setupView(s: DownloadStatus | null): SetupView {
  const hidden: SetupView = { visible: false, title: '', detail: '', percent: null, tone: 'done', canCancel: false, canRetry: false };
  if (!s) return hidden;
  const pct = s.totalBytes > 0 ? Math.min(100, Math.floor((s.doneBytes / s.totalBytes) * 100)) : 0;
  switch (s.state) {
    case 'downloading':
      return {
        visible: true,
        title: `Préparation de Crosstalk · ${pct} %`,
        detail: s.verifying
          ? `${s.current ?? 'Modèle'} · vérification de l’intégrité (SHA-256)`
          : `${s.current ?? 'Téléchargement des modèles'} · ${mb(s.doneBytes)} / ${mb(s.totalBytes)}${s.bytesPerSec ? ` · ${(s.bytesPerSec / 1_000_000).toFixed(1).replace('.', ',')} Mo/s` : ''}`,
        percent: pct,
        tone: 'progress',
        canCancel: true,
        canRetry: false,
      };
    case 'error':
      return { visible: true, title: 'Téléchargement des modèles interrompu', detail: s.error ?? 'Erreur inconnue', percent: pct, tone: 'error', canCancel: false, canRetry: true };
    case 'cancelled':
      return { visible: true, title: 'Téléchargement des modèles annulé', detail: `${mb(s.missingBytes || s.totalBytes - s.doneBytes)} restants · reprise là où il s’est arrêté`, percent: pct, tone: 'error', canCancel: false, canRetry: true };
    default:
      if (s.missingFiles > 0)
        return { visible: true, title: 'Modèles à télécharger', detail: `${s.missingFiles} fichiers · ${mb(s.missingBytes)}`, percent: null, tone: 'progress', canCancel: false, canRetry: true };
      return hidden;
  }
}

/** The models the current settings need: Whisper model, translation on or off. */
export const setupNeeds = (s: Pick<AppState, 'model' | 'tg'>) => ({ model: s.model, translation: s.tg.translate });

export async function startSetup(s: Pick<AppState, 'model' | 'tg'>): Promise<void> {
  const n = setupNeeds(s);
  const status = await modelDownload.start(n.model, n.translation);
  if (status) useSetup.setState({ status });
}
