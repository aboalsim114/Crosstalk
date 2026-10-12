import { useEffect, useState } from 'react';
import { FALLBACK_INFO, autostart, getAppInfo, isTauri, updater, type AppInfo } from '../backend';
import { ToggleRow, fnt } from '../components/atoms';
import { LICENSES, NEVERS } from '../state/data';
import { updateSummary, useUpdate } from '../state/update';

/** Windows integration: launch at login (straight to the notification area). */
function WindowsCard() {
  const [launch, setLaunch] = useState(false);
  useEffect(() => {
    autostart.isEnabled().then(setLaunch, () => setLaunch(false));
  }, []);
  const toggle = async () => {
    const next = !launch;
    setLaunch(next);
    try {
      await autostart.set(next);
    } catch {
      setLaunch(!next);
    }
  };
  return (
    <div className="glass flex flex-col gap-3.5 p-6">
      <div className="lbl">WINDOWS</div>
      <ToggleRow title="Lancer avec Windows" sub="Démarre réduit dans la zone de notification" on={launch} onClick={toggle} />
      <div style={{ font: fnt(400, 11.5, 'sans', 1.6), color: '#6e6961' }}>
        Fermer la fenêtre garde Crosstalk actif dans la zone de notification. Pour quitter : clic droit sur l’icône, puis « Quitter Crosstalk ».
        {!isTauri && ' (Disponible dans l’application de bureau.)'}
      </div>
    </div>
  );
}

function UpdateCard() {
  const { native, status } = useUpdate();
  const busy = status?.state === 'checking' || status?.state === 'downloading' || status?.state === 'installing';
  return (
    <div className="glass flex flex-col gap-2.5 p-6" data-testid="update-card">
      <div className="lbl pb-1">MISES À JOUR</div>
      <span data-testid="update-summary" style={{ font: fnt(400, 13, 'sans', 1.5), color: status?.state === 'error' ? '#ff8a7a' : '#bdb6ab' }}>
        {updateSummary(status)}
      </span>
      {status?.latest && status.latest.build > status.build && status.latest.notes.length > 0 && (
        <ul className="flex flex-col gap-1" style={{ font: fnt(400, 11.5, 'sans', 1.45), color: '#8f8a83' }}>
          {status.latest.notes.map((n) => (
            <li key={n}>· {n}</li>
          ))}
        </ul>
      )}
      {native && (
        <div className="flex gap-2 pt-1">
          <div className="pill px-3.5 py-[7px]" role="button" aria-disabled={busy} onClick={() => !busy && updater.check()} style={{ font: fnt(500, 11.5) }}>
            Rechercher une mise à jour
          </div>
          {status?.state === 'available' && (
            <div className="pill pill-ember px-3.5 py-[7px]" role="button" onClick={() => updater.download()} style={{ font: fnt(500, 11.5) }}>
              Mettre à jour
            </div>
          )}
        </div>
      )}
      <span style={{ font: fnt(400, 10.5, 'sans', 1.5), color: '#5a5650' }}>
        Vérification au lancement puis toutes les 6 h. L’installateur est vérifié (taille, SHA-256) puis installé au même endroit ; modèles et réglages conservés.
      </span>
    </div>
  );
}

export function AboutPage() {
  const upd = useUpdate((x) => x.status);
  const [info, setInfo] = useState<AppInfo>(FALLBACK_INFO);
  useEffect(() => {
    getAppInfo().then(setInfo);
  }, []);
  return (
    <div className="grid max-w-[980px] items-start gap-4" style={{ gridTemplateColumns: 'minmax(0,1.2fr) minmax(0,1fr)' }}>
      <div className="glass flex flex-col gap-3.5 p-6">
        <div style={{ font: fnt(400, 28, 'serif', 1.1), color: '#ece6dc' }}>
          Ce que l’application <span style={{ fontStyle: 'italic', color: '#ff8a7a' }}>ne fait jamais</span>
        </div>
        {NEVERS.map((n) => (
          <div key={n} className="flex gap-3.5 py-1.5" style={{ font: fnt(400, 13, 'sans', 1.5), color: '#bdb6ab', borderBottom: '1px solid rgba(255,255,255,.04)' }}>
            <span style={{ font: fnt(500, 10, 'mono', '20px'), color: '#ff8a7a' }}>✕</span>
            <span>{n}</span>
          </div>
        ))}
        <div style={{ font: fnt(400, 11.5, 'sans', 1.6), color: '#6e6961' }}>
          Aucune garantie « VAC safe » n’est revendiquée. Les interactions externes sont documentées et revues après chaque mise à jour de CS2.
        </div>
      </div>
      <div className="flex flex-col gap-4">
        <div className="glass flex flex-col gap-2 p-6">
          <div className="lbl pb-1">LICENCES TIERCES</div>
          {LICENSES.map(([n, l]) => (
            <div key={n} className="flex justify-between py-[5px]" style={{ font: fnt(400, 12, 'mono'), color: '#bdb6ab', borderBottom: '1px solid rgba(255,255,255,.04)' }}>
              <span>{n}</span>
              <span style={{ color: '#6e6961' }}>{l}</span>
            </div>
          ))}
          <div className="pt-2" style={{ font: fnt(400, 10.5, 'mono'), color: '#5a5650' }}>
            v{info.version}
            {upd ? ` · build ${upd.build} (${upd.commit})` : ''} · Windows x64
          </div>
        </div>
        <UpdateCard />
        <WindowsCard />
      </div>
    </div>
  );
}
