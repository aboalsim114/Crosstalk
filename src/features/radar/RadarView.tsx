import { fnt } from '../../shared/ui/atoms';
import { useT } from '../../shared/i18n';
import { lanes, type MapLayout, type Side } from './layouts';
import { pingLife, type Ping } from './radar';

const VOICE = '#ffa24a';
const CHAT = '#7fd4ff';
const SIDE_DOT: Record<Side, string> = {
  a: 'rgba(255,198,138,.55)',
  b: 'rgba(255,198,138,.55)',
  mid: 'rgba(236,230,220,.4)',
  t: 'rgba(255,138,122,.45)',
  ct: 'rgba(127,212,255,.45)',
};

/** The map plan with the lit zones; `size` is the side of the square plan in pixels. */
export function RadarPlan({ layout, pings, now, hold, size = 172 }: { layout: MapLayout; pings: Ping[]; now: number; hold: number; size?: number }) {
  const lit = new Map(pings.map((p) => [p.zone, p]));
  const pad = 7;
  const at = (v: number) => pad + v * (100 - 2 * pad);
  return (
    <svg viewBox="0 0 100 100" width={size} height={size} role="img" aria-label={layout.name} data-testid="radar-plan" style={{ flex: 'none', overflow: 'visible' }}>
      <rect x="0.5" y="0.5" width="99" height="99" rx="7" fill="rgba(255,255,255,.015)" stroke="rgba(255,255,255,.06)" />
      {lanes(layout).map(([p, q]) => (
        <line key={`${p.id}-${q.id}`} x1={at(p.x)} y1={at(p.y)} x2={at(q.x)} y2={at(q.y)} stroke="rgba(255,255,255,.08)" strokeWidth="0.7" strokeLinecap="round" />
      ))}
      {layout.zones.map((z) => {
        const site = z.id === 'a' || z.id === 'b';
        const spawn = z.id === 't' || z.id === 'ct';
        const p = lit.get(z.id);
        const x = at(z.x);
        const y = at(z.y);
        if (!p) {
          if (site)
            return (
              <g key={z.id}>
                <circle cx={x} cy={y} r="5.6" fill="rgba(255,162,74,.06)" stroke="rgba(255,198,138,.35)" strokeWidth="0.6" />
                <text
                  x={x}
                  y={y + 2}
                  textAnchor="middle"
                  style={{
                    font: fnt(600, 5.6, 'mono'),
                    fill: 'rgba(255,198,138,.7)',
                  }}
                >
                  {z.label}
                </text>
              </g>
            );
          if (spawn)
            return (
              <text key={z.id} x={x} y={y + 1.6} textAnchor="middle" style={{ font: fnt(600, 4.2, 'mono'), fill: SIDE_DOT[z.side] }}>
                {z.label}
              </text>
            );
          return <circle key={z.id} cx={x} cy={y} r="1.1" fill={SIDE_DOT[z.side]} />;
        }
        const color = p.kind === 'voice' ? VOICE : CHAT;
        const life = pingLife(p, now, hold);
        const fresh = now - p.at < 2500;
        const labelBelow = y < 16;
        return (
          <g key={z.id} data-testid={`radar-ping-${z.id}`} style={{ opacity: 0.3 + 0.7 * life, transition: 'opacity .5s' }}>
            {fresh && <circle className="radar-pulse" cx={x} cy={y} r="6" fill="none" stroke={color} strokeWidth="0.8" />}
            <circle cx={x} cy={y} r={site ? 6.2 : 4.2} fill={color} fillOpacity="0.22" stroke={color} strokeWidth="0.9" />
            <circle cx={x} cy={y} r={site ? 2.2 : 1.7} fill={color} />
            <text
              x={x}
              y={labelBelow ? y + 10.5 : y - 6.6}
              textAnchor="middle"
              style={{
                font: fnt(600, 4.6, 'sans'),
                fill: '#fff',
                paintOrder: 'stroke',
                stroke: 'rgba(0,0,0,.85)',
                strokeWidth: 1.4,
              }}
            >
              {z.label}
            </text>
            {p.count !== null && (
              <g>
                <circle cx={x + 5.4} cy={y + 4.4} r="3.6" fill={color} stroke="rgba(0,0,0,.6)" strokeWidth="0.5" />
                <text x={x + 5.4} y={y + 5.9} textAnchor="middle" style={{ font: fnt(700, 4.4, 'mono'), fill: '#120d08' }}>
                  {p.count}
                </text>
              </g>
            )}
          </g>
        );
      })}
    </svg>
  );
}

/** Overlay block: plan on the left, the latest callouts on the right. */
export function RadarPanel({ layout, pings, now, hold, opacity, outline }: { layout: MapLayout; pings: Ping[]; now: number; hold: number; opacity: number; outline?: string }) {
  const t = useT();
  const zoneLabel = (id: string) => layout.zones.find((z) => z.id === id)?.label ?? id;
  return (
    <div
      className="flex w-[380px] gap-3 rounded-[10px] p-3"
      style={{
        background: `rgba(6,6,8,${(opacity / 100).toFixed(2)})`,
        border: '1px solid rgba(255,255,255,.05)',
        outline: outline ?? '1px solid transparent',
        outlineOffset: 8,
      }}
      data-testid="radar-panel"
    >
      <RadarPlan layout={layout} pings={pings} now={now} hold={hold} />
      <div className="flex min-w-0 flex-1 flex-col gap-2">
        <div
          className="truncate"
          style={{
            font: fnt(600, 9.5, 'mono'),
            letterSpacing: '.14em',
            color: VOICE,
          }}
          title={t('radar.title')}
        >
          ◉ {layout.name.toUpperCase()}
        </div>
        {pings.slice(0, 4).map((p) => (
          <div key={p.zone} className="flex items-baseline gap-2" style={{ opacity: 0.45 + 0.55 * pingLife(p, now, hold) }}>
            <span
              className="min-w-0 truncate"
              style={{
                font: fnt(600, 15, 'sans', 1.2),
                color: '#fff',
                textShadow: '0 1px 2px #000',
              }}
            >
              {zoneLabel(p.zone)}
            </span>
            {p.count !== null && (
              <span
                style={{
                  font: fnt(700, 15, 'mono'),
                  color: p.kind === 'voice' ? VOICE : CHAT,
                }}
              >
                ×{p.count}
              </span>
            )}
            <span className="ml-auto flex-none" style={{ font: fnt(400, 10, 'mono'), color: '#8f8a83' }}>
              {t('radar.ago', {
                n: Math.max(0, Math.round((now - p.at) / 1000)),
              })}
            </span>
          </div>
        ))}
      </div>
    </div>
  );
}
