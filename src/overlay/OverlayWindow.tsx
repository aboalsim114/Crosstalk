import { useEffect, useState } from 'react';
import { AnimatePresence, motion } from 'motion/react';
import { overlayWindow } from '../backend';
import type { OverlayPayload } from '../native';
import { OverlayStack } from '../pages/OverlayPage';
import type { FeedEntry } from '../state/store';

/** Lines younger than `duration` seconds. Feed ids are arrival timestamps (ms). */
export function visibleLines(feed: FeedEntry[], duration: number, now: number): FeedEntry[] {
  return feed.filter((f) => now - f.id < duration * 1000);
}

/** Content of the transparent, click-through, always-on-top overlay window. */
export function OverlayWindow({ initial }: { initial?: OverlayPayload }) {
  const [s, setS] = useState<OverlayPayload | null>(initial ?? null);
  const [now, setNow] = useState(() => Date.now());

  useEffect(() => {
    const un = overlayWindow.onState<OverlayPayload>(setS);
    overlayWindow.announceReady();
    const iv = setInterval(() => setNow(Date.now()), 500);
    return () => {
      un.then((f) => f());
      clearInterval(iv);
    };
  }, []);

  if (!s) return null;
  // While repositioning, show sample lines so the area is visible; otherwise only fresh lines.
  const feed = s.repos ? [] : visibleLines(s.feed, s.duration, now);
  const show = s.repos || feed.length > 0;
  const bottom = s.corner[0] === 'b';
  const right = s.corner[1] === 'r';

  return (
    <div
      className="flex h-screen w-screen p-3"
      style={{ alignItems: bottom ? 'flex-end' : 'flex-start', justifyContent: right ? 'flex-end' : 'flex-start' }}
      data-tauri-drag-region={s.repos || undefined}
    >
      <AnimatePresence>
        {show && (
          <motion.div
            key="stack"
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            transition={{ duration: 0.4 }}
            className={s.repos ? 'cursor-move' : undefined}
            data-tauri-drag-region={s.repos || undefined}
          >
            <OverlayStack s={{ ...s, feed }} outline={s.repos ? '1px dashed #ffa24a' : undefined} />
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}
