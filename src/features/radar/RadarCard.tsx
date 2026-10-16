import { useEffect, useState } from 'react';
import { Slider, ToggleRow, fnt } from '../../shared/ui/atoms';
import { useT } from '../../shared/i18n';
import { useApp } from '../../app/state/store';
import { useGsi } from '../recap/gsi';
import { RadarPanel } from './RadarView';
import { HOLD_RANGE, layoutOf, pingLife, radarPayload, samplePings } from './radar';

/** Overlay › callout radar: switch, how long a zone stays lit, live preview and a sample. */
export function RadarCard() {
  const t = useT();
  const radar = useApp((s) => s.radar);
  const feed = useApp((s) => s.feed);
  const demo = useApp((s) => s.radarDemo);
  const opacity = useApp((s) => s.opacity);
  const set = useApp((s) => s.set);
  const map = useGsi((s) => s.status?.map ?? null);
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    const iv = setInterval(() => setNow(Date.now()), 500);
    return () => clearInterval(iv);
  }, []);

  const live = radarPayload({ ...radar, on: true }, map, feed, demo, now);
  const pings = live?.pings.filter((p) => pingLife(p, now, radar.hold) > 0) ?? [];
  // Before any callout, the preview shows the sample so the plan is visible.
  const shown = live && pings.length ? { ...live, pings } : { ...samplePings(now), hold: radar.hold };
  const layout = layoutOf(shown.map) ?? layoutOf('de_mirage')!;
  const mapLayout = layoutOf(map);

  return (
    <div className="glass flex flex-col gap-3.5 p-5" data-testid="radar-card">
      <div className="flex items-center justify-between">
        <span className="lbl">{t('radar.title')}</span>
        <span
          style={{
            font: fnt(500, 9.5, 'mono'),
            letterSpacing: '.12em',
            color: mapLayout ? '#7fe0a8' : '#6e6961',
          }}
        >
          {mapLayout ? t('radar.map', { map: mapLayout.name }) : t('radar.sample')}
        </span>
      </div>
      <div className="grid items-start gap-5" style={{ gridTemplateColumns: 'minmax(0,1fr) 330px' }}>
        <div className="flex flex-col gap-2">
          <div style={{ font: fnt(400, 11.5, 'sans', 1.5), color: '#8f8a83' }}>{t('radar.lead')}</div>
          <div style={{ opacity: radar.on ? 1 : 0.45, transition: 'opacity .3s' }}>
            <RadarPanel layout={layout} pings={shown.pings} now={now} hold={radar.hold} opacity={Math.max(opacity, 60)} />
          </div>
          <span style={{ font: fnt(400, 10.5, 'sans', 1.5), color: '#6e6961' }}>{t('radar.note')}</span>
        </div>
        <div className="flex flex-col gap-3.5">
          <ToggleRow title={t('radar.toggle')} sub={t('radar.toggleSub')} on={radar.on} onClick={() => set({ radar: { ...radar, on: !radar.on } })} />
          <Slider label={t('radar.hold')} unit="s" min={HOLD_RANGE[0]} max={HOLD_RANGE[1]} value={radar.hold} onChange={(v) => set({ radar: { ...radar, hold: v } })} />
          {!mapLayout && (
            <span style={{ font: fnt(400, 11, 'sans', 1.5), color: '#ffa24a' }} data-testid="radar-no-map">
              {t('radar.noMap')}
            </span>
          )}
          <div
            role="button"
            aria-disabled={!radar.on}
            onClick={() => radar.on && set({ radarDemo: Date.now() })}
            className="cursor-pointer rounded-full py-2.5 text-center"
            style={{
              border: '1px solid rgba(255,162,74,.35)',
              font: fnt(500, 12.5),
              color: '#ffc68a',
              opacity: radar.on ? 1 : 0.4,
            }}
          >
            {t('radar.try')}
          </div>
        </div>
      </div>
    </div>
  );
}
