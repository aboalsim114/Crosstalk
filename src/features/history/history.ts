// Local history: when the user turns it on, every real line that reached its
// final state (translated, kept in French, or left untranslated with a reason)
// is written by the Rust core to %LOCALAPPDATA%\Crosstalk\history.jsonl.
// Text only, never audio; purged after 7 days (src-tauri/src/history.rs).

import { historyStore, type HistoryEntry } from '../../shared/ipc';
import { useApp, type FeedEntry } from '../../app/state/store';

/** The history line of a feed entry, once it will not change any more. */
export function historyEntryFrom(f: FeedEntry, now = Date.now()): HistoryEntry | null {
  if (f.tid == null || f.live) return null; // demo or live line
  const done = f.status === 'final' || f.status === 'original' || f.status === 'erreur' || (f.status === 'transcrit' && !!f.note);
  if (!done) return null;
  return {
    ts: now,
    kind: f.kind,
    ...(f.kind === 'chat' && f.ch ? { channel: f.ch } : {}),
    ...(f.sender ? { sender: f.sender } : {}),
    lang: f.src,
    original: f.orig,
    ...(f.status === 'final' && f.shown ? { translated: f.shown } : {}),
    status: f.status,
  };
}

/** Records final lines while history is on; returns the unsubscribe. */
export function startHistoryRecorder(append: (e: HistoryEntry) => Promise<void> = historyStore.append): () => void {
  const recorded = new Set<number>();
  return useApp.subscribe((s, prev) => {
    if (s.feed === prev.feed || !s.tg.hist) return;
    for (const f of s.feed) {
      if (f.tid == null || recorded.has(f.tid)) continue;
      const e = historyEntryFrom(f);
      if (!e) continue;
      recorded.add(f.tid);
      append(e).catch((err) => console.warn('history_append failed', err));
    }
  });
}
