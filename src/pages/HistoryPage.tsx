import { useEffect, useState } from 'react';
import { historyStore, isTauri, type HistoryEntry } from '../backend';
import { Toggle, fnt } from '../components/atoms';
import { CHANNEL_NAME, type Channel } from '../state/data';
import { useApp } from '../state/store';
import { RecapPanel } from './RecapPanel';

const COLS = '128px 120px minmax(0,1fr) minmax(0,1fr)';

function when(ts: number): string {
  const d = new Date(ts);
  return `${d.toLocaleDateString('fr-FR', { day: '2-digit', month: '2-digit' })} ${d.toLocaleTimeString('fr-FR')}`;
}

/** One stored line as a row. */
export function historyRow(e: HistoryEntry) {
  const voice = e.kind === 'voice';
  return {
    when: when(e.ts),
    tag: voice ? `VOIX · ${e.lang.toUpperCase()}` : `CHAT · ${CHANNEL_NAME[e.channel as Channel] ?? e.channel ?? ''}`,
    color: voice ? '#ffa24a' : '#7fd4ff',
    original: (e.sender ? `${e.sender} — ` : '') + e.original,
    shown: e.translated ?? (e.status === 'original' ? 'Déjà en français' : 'Non traduit'),
    muted: !e.translated,
  };
}

export function HistoryPage() {
  const on = useApp((s) => s.tg.hist);
  const feed = useApp((s) => s.feed);
  const { toggle, flash } = useApp.getState();
  const [query, setQuery] = useState('');
  const [rows, setRows] = useState<HistoryEntry[]>([]);
  const [total, setTotal] = useState(0);

  // Reload on search, on toggle and whenever new lines arrive (debounced).
  useEffect(() => {
    if (!isTauri) return;
    const t = setTimeout(() => {
      historyStore.list(query, 500).then(setRows);
      historyStore.count().then(setTotal);
    }, 250);
    return () => clearTimeout(t);
  }, [query, on, feed]);

  const exportTo = (format: 'txt' | 'json') =>
    historyStore.exportTo(format).then(
      (path) => flash(`Exporté : ${path}`),
      (e) => flash(`Export impossible : ${e}`),
    );
  const clear = () =>
    historyStore.clear().then(() => {
      setRows([]);
      setTotal(0);
      flash('Historique effacé');
    });

  return (
    <div className="flex max-w-[980px] flex-col gap-4">
      <div className="glass flex items-center justify-between gap-4 p-5">
        <div className="flex flex-col gap-1">
          <span style={{ font: fnt(400, 22, 'serif'), color: '#ece6dc' }}>Historique local</span>
          <span style={{ font: fnt(400, 12, 'sans', 1.5), color: '#6e6961' }}>
            Désactivé par défaut. Texte uniquement, jamais d’audio. Fichier sur ce PC (%LOCALAPPDATA%\Crosstalk\history.jsonl), effacé au bout de 7 jours.
          </span>
        </div>
        <Toggle on={on} onClick={() => toggle('hist')} label="Historique local" />
      </div>
      <RecapPanel />
      {!isTauri && (
        <div className="glass p-5" style={{ font: fnt(400, 12.5, 'sans', 1.5), color: '#6e6961' }}>
          Dans l’application Windows, les sous-titres de vos parties sont enregistrés ici quand l’historique est activé.
        </div>
      )}
      {isTauri && (on || total > 0) && (
        <div className="glass flex flex-col overflow-hidden" data-testid="history">
          <div className="flex items-center gap-2 px-5 py-3.5" style={{ borderBottom: '1px solid rgba(255,255,255,.05)' }}>
            <input className="text-input min-w-0 flex-1" aria-label="Rechercher dans l’historique" placeholder="Rechercher un mot, une phrase, un pseudo…" value={query} onChange={(e) => setQuery(e.target.value)} />
            <span style={{ font: fnt(400, 10.5, 'mono'), color: '#5a5650' }}>{total} lignes</span>
            {(['txt', 'json'] as const).map((x) => (
              <div key={x} className="pill px-3.5 py-2" role="button" aria-disabled={!total} onClick={() => total && exportTo(x)} style={{ font: fnt(500, 12) }}>
                {x.toUpperCase()}
              </div>
            ))}
            <div className="pill pill-danger px-3.5 py-2" role="button" aria-disabled={!total} onClick={() => total && clear()} style={{ font: fnt(500, 12) }}>
              Effacer
            </div>
          </div>
          {!rows.length && (
            <div className="p-10 text-center" style={{ font: fnt(400, 18, 'serif'), fontStyle: 'italic', color: '#6e6961' }}>
              {query ? 'Aucun résultat.' : on ? 'Aucune ligne pour l’instant : elles s’ajoutent pendant vos parties.' : 'Historique en pause.'}
            </div>
          )}
          {rows.map((e, i) => {
            const r = historyRow(e);
            return (
              <div key={`${e.ts}-${i}`} className="grid items-baseline gap-3.5 px-5 py-2.5" style={{ gridTemplateColumns: COLS, borderBottom: '1px solid rgba(255,255,255,.035)' }}>
                <span style={{ font: fnt(400, 11, 'mono'), color: '#5a5650' }}>{r.when}</span>
                <span style={{ font: fnt(600, 9.5, 'mono'), letterSpacing: '.12em', color: r.color }}>{r.tag}</span>
                <span style={{ font: fnt(400, 12, 'mono'), color: '#8f8a83' }}>{r.original}</span>
                <span style={{ font: fnt(400, 13), color: r.muted ? '#6e6961' : '#ece6dc', fontStyle: r.muted ? 'italic' : 'normal' }}>{r.shown}</span>
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}
