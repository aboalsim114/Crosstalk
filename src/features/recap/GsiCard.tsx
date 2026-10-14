import { useState } from 'react';
import { gsi } from '../../shared/ipc';
import { fnt } from '../../shared/ui/atoms';
import { gsiSummary, useGsi } from './gsi';
import { useApp } from '../../app/state/store';

/** Map being played (Game State Integration): enable, state, callouts. */
export function GsiCard() {
  const st = useGsi((x) => x.status);
  const flash = useApp((s) => s.flash);
  const [busy, setBusy] = useState(false);
  const sum = gsiSummary(st);
  const act = (f: () => Promise<unknown>) => {
    setBusy(true);
    f()
      .catch((e) => flash(String(e)))
      .finally(() => setBusy(false));
  };
  return (
    <div className="glass flex flex-col gap-3.5 p-5" data-testid="gsi-card">
      <div className="flex items-baseline justify-between">
        <span className="lbl">CARTE EN COURS · GAME STATE INTEGRATION</span>
        {st?.mode && <span style={{ font: fnt(400, 10, 'mono'), color: '#5a5650' }}>{st.mode}</span>}
      </div>
      <div className="flex items-start gap-2.5">
        <div className="mt-[7px] h-1.5 w-1.5 flex-none rounded-[3px]" style={{ background: sum.color, boxShadow: `0 0 8px ${sum.color}` }} />
        <div className="flex min-w-0 flex-col gap-0.5">
          <span data-testid="gsi-state" style={{ font: fnt(500, 13), color: '#ece6dc' }}>
            {sum.value}
          </span>
          <span style={{ font: fnt(400, 11, 'sans', 1.45), color: '#6e6961' }}>{sum.detail}</span>
        </div>
      </div>
      <span style={{ font: fnt(400, 11, 'sans', 1.5), color: '#6e6961' }}>
        Interface officielle de Valve : Crosstalk ajoute un fichier <span style={{ fontFamily: 'var(--font-mono)' }}>gamestate_integration_crosstalk.cfg</span> dans le dossier cfg de CS2, et CS2 lui
        envoie seulement le nom de la carte, en local. Aucune lecture du jeu, compatible VAC.
      </span>
      {st?.cs2Found && (
        <div className="flex gap-2">
          {st.installed ? (
            <div className="pill px-4 py-[7px]" role="button" aria-disabled={busy} onClick={() => !busy && act(gsi.uninstall)} style={{ font: fnt(500, 12) }}>
              Retirer
            </div>
          ) : (
            <div className="pill pill-ember px-4 py-[7px]" role="button" aria-disabled={busy} onClick={() => !busy && act(gsi.install)} style={{ font: fnt(500, 12) }}>
              Activer
            </div>
          )}
        </div>
      )}
    </div>
  );
}
