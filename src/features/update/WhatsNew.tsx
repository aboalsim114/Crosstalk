import { useEffect, useState } from 'react';
import type { NewsEntry, UpdateStatus } from '../../shared/ipc';
import { fnt } from '../../shared/ui/atoms';
import { formatDate, newsSince, useUpdate, versionLabel } from './update';

const SEEN_KEY = 'crosstalk.news.seenBuild';

/** The last version whose news the player saw (null: never stored). */
export function readSeen(): number | null {
  try {
    const v = Number(localStorage.getItem(SEEN_KEY));
    return Number.isInteger(v) && v > 0 ? v : null;
  } catch {
    return null;
  }
}

export function writeSeen(build: number) {
  try {
    localStorage.setItem(SEEN_KEY, String(build));
  } catch {
    // Private storage unavailable: the news may show again, nothing breaks.
  }
}

/**
 * News to show once after an update. Never stored yet (first launch with this
 * feature, or a fresh install): only what the installed version itself brings.
 */
export function whatsNew(st: UpdateStatus | null, seen: number | null): NewsEntry[] | null {
  if (!st?.latest?.news) return null; // feed not read yet
  return newsSince(st, seen ?? st.build - 1);
}

/** The changes, as a player reads them: a title, then plain sentences. */
export function NewsList({ entries, dates = true }: { entries: NewsEntry[]; dates?: boolean }) {
  return (
    <div className="flex flex-col gap-4">
      {entries.map((n) => (
        <div key={`${n.build}-${n.title}`} className="flex flex-col gap-1.5">
          <div className="flex items-baseline justify-between gap-3">
            <span style={{ font: fnt(500, 13.5), color: '#ece6dc' }}>{n.title}</span>
            {dates && <span className="flex-none" style={{ font: fnt(400, 10.5, 'mono'), color: '#5a5650' }}>{formatDate(n.date)}</span>}
          </div>
          <ul className="flex flex-col gap-1">
            {n.items.map((i) => (
              <li key={i} className="flex gap-2" style={{ font: fnt(400, 12, 'sans', 1.5), color: '#a9a297' }}>
                <span style={{ color: '#ff8a7a' }}>•</span>
                <span>{i}</span>
              </li>
            ))}
          </ul>
        </div>
      ))}
    </div>
  );
}

/** "What's new" after an update, shown once per version. */
export function WhatsNew() {
  const status = useUpdate((s) => s.status);
  const [entries, setEntries] = useState<NewsEntry[] | null>(null);
  useEffect(() => {
    if (!status || entries) return;
    const found = whatsNew(status, readSeen());
    if (found == null) return;
    if (found.length) setEntries(found);
    else writeSeen(status.build);
  }, [status, entries]);
  if (!entries?.length || !status) return null;
  const close = () => {
    writeSeen(status.build);
    setEntries([]);
  };
  return (
    <div
      className="absolute inset-0 z-[55] flex items-center justify-center"
      style={{ background: 'rgba(8,7,6,.78)', backdropFilter: 'blur(6px)' }}
      role="dialog"
      aria-modal="true"
      aria-label="Nouveautés"
      onKeyDown={(e) => e.key === 'Escape' && close()}
    >
      <div className="glass flex max-h-[86%] w-[560px] max-w-[92%] flex-col gap-5 p-7" style={{ background: 'rgba(18,16,14,.94)' }}>
        <div className="flex flex-col gap-1.5">
          <span className="lbl">VERSION {versionLabel(status.version, status.build)}</span>
          <span style={{ font: fnt(400, 30, 'serif', 1.1), color: '#ece6dc' }}>
            Quoi de <span style={{ fontStyle: 'italic', color: '#ff8a7a' }}>neuf</span> ?
          </span>
          <span style={{ font: fnt(400, 12.5, 'sans', 1.55), color: '#8f8a83' }}>Crosstalk vient d’être mis à jour. Voici ce qui change pour vous.</span>
        </div>
        <div className="min-h-0 overflow-auto pr-1">
          <NewsList entries={entries} />
        </div>
        <div className="flex justify-end">
          <div className="pill pill-ember px-4 py-2" role="button" autoFocus tabIndex={0} onClick={close} onKeyDown={(e) => e.key === 'Enter' && close()} style={{ font: fnt(500, 12) }}>
            C’est parti
          </div>
        </div>
      </div>
    </div>
  );
}
