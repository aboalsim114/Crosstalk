import { updater } from '../../shared/ipc';
import { updateView, useUpdate } from './update';
import { isRunning, useApp } from '../../app/state/store';
import { fnt } from '../../shared/ui/atoms';

/** "New version" banner on every page: download, then restart to install. */
export function UpdateBanner() {
  const { status, dismissed } = useUpdate();
  const running = useApp(isRunning);
  const v = updateView(status, dismissed, running);
  if (!v.banner) return null;
  const run = () => {
    if (v.action === 'download' || v.action === 'retry') updater.download().then((ok) => {
        if (!ok) updater.check();
      });
    if (v.action === 'install' && !v.blocked) updater.install();
  };
  return (
    <div className="glass mx-8 mb-3 flex flex-col gap-2 px-5 py-3.5" role="status" data-testid="update-banner">
      <div className="flex items-center justify-between gap-4">
        <div className="flex min-w-0 flex-col gap-0.5">
          <span style={{ font: fnt(500, 13), color: '#ece6dc' }}>{v.title}</span>
          <span className="truncate" style={{ font: fnt(400, 11.5), color: '#8f8a83' }}>
            {v.detail}
          </span>
        </div>
        <div className="flex flex-none gap-2">
          {status?.state === 'downloading' && (
            <div className="pill pill-ghost px-3.5 py-[7px]" role="button" onClick={() => updater.cancel()} style={{ font: fnt(500, 11.5) }}>
              Annuler
            </div>
          )}
          {(status?.state === 'available' || status?.state === 'error') && (
            <div className="pill pill-ghost px-3.5 py-[7px]" role="button" onClick={() => useUpdate.setState({ dismissed: status.latest?.build ?? null })} style={{ font: fnt(500, 11.5) }}>
              Plus tard
            </div>
          )}
          {v.action && (
            <div className="pill pill-ember px-3.5 py-[7px]" role="button" aria-disabled={v.blocked} onClick={run} style={{ font: fnt(500, 11.5), opacity: v.blocked ? 0.5 : 1 }}>
              {v.action === 'download' ? 'Mettre à jour' : v.action === 'install' ? 'Redémarrer et installer' : 'Réessayer'}
            </div>
          )}
        </div>
      </div>
      {v.percent != null && (
        <div className="h-1 overflow-hidden rounded-full" style={{ background: 'rgba(255,255,255,.08)' }}>
          <div data-testid="update-progress" style={{ width: `${v.percent}%`, height: '100%', background: '#7fe0a8', transition: 'width .4s' }} />
        </div>
      )}
    </div>
  );
}
