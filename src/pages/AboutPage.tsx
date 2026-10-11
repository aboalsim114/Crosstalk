import { useEffect, useState } from 'react';
import { FALLBACK_INFO, getAppInfo, type AppInfo } from '../backend';
import { fnt } from '../components/atoms';
import { LICENSES, NEVERS } from '../state/data';

export function AboutPage() {
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
      <div className="glass flex flex-col gap-2 p-6">
        <div className="lbl pb-1">LICENCES TIERCES</div>
        {LICENSES.map(([n, l]) => (
          <div key={n} className="flex justify-between py-[5px]" style={{ font: fnt(400, 12, 'mono'), color: '#bdb6ab', borderBottom: '1px solid rgba(255,255,255,.04)' }}>
            <span>{n}</span>
            <span style={{ color: '#6e6961' }}>{l}</span>
          </div>
        ))}
        <div className="pt-2" style={{ font: fnt(400, 10.5, 'mono'), color: '#5a5650' }}>
          v{info.version} · build 2026-10-11 · Windows x64
        </div>
      </div>
    </div>
  );
}
