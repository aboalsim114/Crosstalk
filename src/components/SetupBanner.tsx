import { modelDownload } from '../backend';
import { setupView, startSetup, useSetup } from '../state/setup';
import { useApp } from '../state/store';
import { fnt } from './atoms';

/** First-run download of the models, on every page until it is complete. */
export function SetupBanner() {
  const status = useSetup((s) => s.status);
  const v = setupView(status);
  if (!v.visible) return null;
  const color = v.tone === 'error' ? '#ff8a7a' : '#ffa24a';
  return (
    <div className="glass mx-8 mb-3 flex flex-col gap-2 px-5 py-3.5" role="status" data-testid="setup-banner">
      <div className="flex items-center justify-between gap-4">
        <div className="flex min-w-0 flex-col gap-0.5">
          <span style={{ font: fnt(500, 13), color: '#ece6dc' }}>{v.title}</span>
          <span className="truncate" style={{ font: fnt(400, 11.5), color: '#8f8a83' }}>
            {v.detail}
          </span>
        </div>
        <div className="flex flex-none gap-2">
          {v.canCancel && (
            <div className="pill pill-ghost px-3.5 py-[7px]" role="button" onClick={() => modelDownload.cancel()} style={{ font: fnt(500, 11.5) }}>
              Annuler
            </div>
          )}
          {v.canRetry && (
            <div className="pill pill-ember px-3.5 py-[7px]" role="button" onClick={() => startSetup(useApp.getState())} style={{ font: fnt(500, 11.5) }}>
              {status?.state === 'idle' ? 'Télécharger' : 'Reprendre'}
            </div>
          )}
        </div>
      </div>
      {v.percent != null && (
        <div className="h-1 overflow-hidden rounded-full" style={{ background: 'rgba(255,255,255,.08)' }}>
          <div data-testid="setup-progress" style={{ width: `${v.percent}%`, height: '100%', background: color, transition: 'width .4s' }} />
        </div>
      )}
      <span style={{ font: fnt(400, 10.5), color: '#5a5650' }}>Une seule fois · Hugging Face · fichiers vérifiés (taille, SHA-1 / SHA-256) · ensuite tout fonctionne hors ligne</span>
    </div>
  );
}
