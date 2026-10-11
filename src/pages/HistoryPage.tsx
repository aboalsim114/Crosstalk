import { Toggle, fnt } from '../components/atoms';
import { useApp } from '../state/store';
import { feedRow } from './derive';

export function HistoryPage() {
  const on = useApp((s) => s.tg.hist);
  const feed = useApp((s) => s.feed);
  const { toggle, clearFeed } = useApp.getState();
  return (
    <div className="flex max-w-[860px] flex-col gap-4">
      <div className="glass flex items-center justify-between gap-4 p-5">
        <div className="flex flex-col gap-1">
          <span style={{ font: fnt(400, 22, 'serif'), color: '#ece6dc' }}>Historique local</span>
          <span style={{ font: fnt(400, 12, 'sans', 1.5), color: '#6e6961' }}>Désactivé par défaut. Texte uniquement, jamais d’audio. SQLite sur ce PC, purge après 7 jours.</span>
        </div>
        <Toggle on={on} onClick={() => toggle('hist')} label="Historique local" />
      </div>
      {on && (
        <div className="glass flex flex-col overflow-hidden">
          <div className="flex gap-2 px-5 py-3.5" style={{ borderBottom: '1px solid rgba(255,255,255,.05)' }}>
            <div className="flex-1 rounded-full px-3.5 py-2" style={{ border: '1px solid rgba(255,255,255,.08)', font: fnt(400, 12.5), color: '#5a5650' }}>
              Rechercher…
            </div>
            {['TXT', 'JSON'].map((x) => (
              <div key={x} className="rounded-full px-3.5 py-2" style={{ border: '1px solid rgba(255,255,255,.08)', font: fnt(500, 12), color: '#bdb6ab' }}>
                {x}
              </div>
            ))}
            <div className="pill pill-danger px-3.5 py-2" role="button" onClick={clearFeed} style={{ font: fnt(500, 12) }}>
              Effacer
            </div>
          </div>
          {!feed.length && (
            <div className="p-10 text-center" style={{ font: fnt(400, 18, 'serif'), fontStyle: 'italic', color: '#6e6961' }}>
              Aucune entrée pour l’instant.
            </div>
          )}
          {feed.map((f) => {
            const e = feedRow(f);
            return (
              <div
                key={e.id}
                className="grid items-baseline gap-3.5 px-5 py-2.5"
                style={{ gridTemplateColumns: '70px 90px minmax(0,1fr) minmax(0,1fr)', borderBottom: '1px solid rgba(255,255,255,.035)' }}
              >
                <span style={{ font: fnt(400, 11, 'mono'), color: '#5a5650' }}>{e.time}</span>
                <span style={{ font: fnt(600, 9.5, 'mono'), letterSpacing: '.12em', color: e.color }}>{e.tag}</span>
                <span style={{ font: fnt(400, 12, 'mono'), color: '#6e6961' }}>{e.orig}</span>
                <span style={{ font: fnt(400, 13), color: '#ece6dc' }}>{e.shown}</span>
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}
