// Mode streamer: final subtitles sent to the OBS page served by the Rust core
// (src-tauri/src/stream). Off by default; names and originals are filtered
// in Rust according to the user's choices.

import { streamOverlay, type StreamLine } from '../../shared/ipc';
import { historyEntryFrom } from '../history/history';
import { useApp, type FeedEntry } from '../../app/state/store';

/** The stream line of a feed entry: translated, or already in French. */
export function streamLineFrom(f: FeedEntry): StreamLine | null {
  const e = historyEntryFrom(f);
  if (!e || (e.status !== 'final' && e.status !== 'original')) return null;
  return {
    kind: e.kind,
    lang: e.lang,
    ...(e.channel ? { channel: e.channel } : {}),
    ...(e.sender ? { sender: e.sender } : {}),
    original: e.original,
    ...(e.translated ? { translated: e.translated } : {}),
  };
}

export const TEST_LINE: StreamLine = { kind: 'voice', lang: 'ru', original: 'Двое на Б, один лоу', translated: 'Deux sur B, un à peu de PV', test: true };

/** Keeps the Rust options in sync and pushes each final line once. */
export function startStreamBridge(push: (l: StreamLine) => Promise<boolean> = streamOverlay.push): () => void {
  void streamOverlay.configure(useApp.getState().stream);
  const sent = new Set<number>();
  return useApp.subscribe((s, prev) => {
    if (s.stream !== prev.stream) void streamOverlay.configure(s.stream);
    if (s.feed === prev.feed || !s.stream.enabled) return;
    for (const f of s.feed) {
      if (f.tid == null || sent.has(f.tid)) continue;
      const line = streamLineFrom(f);
      if (!line) continue;
      sent.add(f.tid);
      push(line).catch(() => {});
    }
  });
}
