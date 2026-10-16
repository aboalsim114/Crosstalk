import { useEffect, useState } from 'react';
import { AnimatePresence, motion } from 'motion/react';
import { overlayWindow } from '../../shared/ipc';
import { useLocale, useTargetLang } from '../../shared/i18n';
import type { OverlayPayload } from '../../app/native';
import { OverlayStack } from '../../features/overlay/OverlayPage';
import { arrivedAt, type FeedEntry } from '../../app/state/store';
import { RadarPanel } from '../../features/radar/RadarView';
import { layoutOf, pingLife, samplePings, type RadarPayload } from '../../features/radar/radar';

/** Radar to draw: the sample while repositioning, otherwise only while a zone is still lit. */
export function visibleRadar(r: RadarPayload | null | undefined, repos: boolean, now: number): RadarPayload | null {
  if (!r) return null;
  if (repos) return { ...samplePings(now), hold: r.hold };
  const pings = r.pings.filter((p) => pingLife(p, now, r.hold) > 0);
  return pings.length && layoutOf(r.map) ? { ...r, pings } : null;
}

/** Lines younger than `duration` seconds (arrival time, see `arrivedAt`). */
export function visibleLines(feed: FeedEntry[], duration: number, now: number): FeedEntry[] {
  return feed.filter((f) => now - arrivedAt(f) < duration * 1000);
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

  // Labels in the main window's language (this window has its own copy of the store).
  useEffect(() => {
    if (s?.locale && useLocale.getState().locale !== s.locale) useLocale.setState({ locale: s.locale });
    if (s?.target && useTargetLang.getState().target !== s.target) useTargetLang.setState({ target: s.target });
  }, [s?.locale, s?.target]);

  if (!s) return null;
  // While repositioning, show sample lines so the area is visible; otherwise only fresh lines.
  const feed = s.repos ? [] : visibleLines(s.feed, s.duration, now);
  const radar = visibleRadar(s.radar, s.repos, now);
  const radarLayout = radar ? layoutOf(radar.map) : null;
  const show = s.repos || feed.length > 0;
  const bottom = s.corner[0] === 'b';
  const right = s.corner[1] === 'r';

  return (
    <div
      className="flex h-screen w-screen p-3"
      style={{ alignItems: bottom ? 'flex-end' : 'flex-start', justifyContent: right ? 'flex-end' : 'flex-start' }}
      data-tauri-drag-region={s.repos || undefined}
    >
      <div className="flex flex-col gap-2" style={{ alignItems: right ? 'flex-end' : 'flex-start' }}>
        <AnimatePresence>
          {radar && radarLayout && (
            <motion.div key="radar" initial={{ opacity: 0, y: bottom ? 8 : -8 }} animate={{ opacity: 1, y: 0 }} exit={{ opacity: 0 }} transition={{ duration: 0.35 }} style={{ order: bottom ? 0 : 1 }}>
              <RadarPanel layout={radarLayout} pings={radar.pings} now={now} hold={radar.hold} opacity={s.opacity} outline={s.repos ? '1px dashed #ffa24a' : undefined} />
            </motion.div>
          )}
          {show && (
            <motion.div key="stack" initial={{ opacity: 0 }} animate={{ opacity: 1 }} exit={{ opacity: 0 }} transition={{ duration: 0.4 }} className={s.repos ? 'cursor-move' : undefined} data-tauri-drag-region={s.repos || undefined}>
              <OverlayStack s={{ ...s, feed }} outline={s.repos ? '1px dashed #ffa24a' : undefined} />
            </motion.div>
          )}
        </AnimatePresence>
      </div>
    </div>
  );
}
