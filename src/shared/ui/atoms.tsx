import type { CSSProperties, ReactNode } from 'react';

const ON = '#ffa24a';
const OFF = 'rgba(255,255,255,.12)';

type Family = 'sans' | 'serif' | 'mono';

/** CSS `font` shorthand with the design's families, e.g. fnt(500, 10, 'mono'). */
export const fnt = (weight: number | string, size: number, family: Family = 'sans', lineHeight?: number | string) =>
  `${weight} ${size}px${lineHeight !== undefined ? `/${lineHeight}` : ''} var(--font-${family})`;

/** Cinematic ease used throughout the mockup. */
export const EASE: [number, number, number, number] = [0.2, 0.7, 0.1, 1];

export const fmtSeconds = (ms: number) => (ms / 1000).toFixed(2).replace('.', ',') + ' s';

export function Toggle({ on, onClick, label }: { on: boolean; onClick: () => void; label?: string }) {
  return (
    <div className="toggle" role="switch" aria-checked={on} aria-label={label} onClick={onClick} style={{ background: on ? ON : OFF }}>
      <div style={{ left: on ? 18 : 2 }} />
    </div>
  );
}

/** Title + sub line on the left, toggle on the right. */
export function ToggleRow({ title, sub, subMono, on, onClick, label }: { title: ReactNode; sub: ReactNode; subMono?: boolean; on: boolean; onClick: () => void; label?: string }) {
  return (
    <div className="flex items-center justify-between gap-3">
      <div className="flex flex-col gap-[3px]">
        <span className="row-title">{title}</span>
        <span className="row-sub" style={subMono ? { fontFamily: 'var(--font-mono)' } : undefined}>
          {sub}
        </span>
      </div>
      <Toggle on={on} onClick={onClick} label={label ?? (typeof title === 'string' ? title : undefined)} />
    </div>
  );
}

export interface SegProps<T extends string | number> {
  options: [T, string][];
  value: T;
  onChange: (v: T) => void;
  /** Per-item style: padding and font differ between instances in the design. */
  item: CSSProperties;
  /** Equal-width columns instead of content-sized items. */
  grid?: number;
}

export function Seg<T extends string | number>({ options, value, onChange, item, grid }: SegProps<T>) {
  return (
    <div className="seg" role="radiogroup" style={grid ? { display: 'grid', gridTemplateColumns: `repeat(${grid},1fr)` } : undefined}>
      {options.map(([v, label]) => (
        <div
          key={String(v)}
          role="radio"
          aria-checked={value === v}
          onClick={() => onChange(v)}
          style={{ ...item, background: value === v ? 'rgba(255,255,255,.1)' : 'transparent', color: value === v ? '#ece6dc' : '#6e6961' }}
        >
          {label}
        </div>
      ))}
    </div>
  );
}

export function Slider({ label, unit, value, min, max, onChange, hint }: { label: string; unit: string; value: number; min: number; max: number; onChange: (v: number) => void; hint?: string }) {
  return (
    <div className="flex flex-col gap-1.5">
      <div className="slider-head">
        <span>{label}</span>
        <span>
          {value} {unit}
        </span>
      </div>
      <input type="range" aria-label={label} min={min} max={max} value={value} onChange={(e) => onChange(+e.target.value)} />
      {hint && <span style={{ font: fnt(400, 11), color: '#5a5650' }}>{hint}</span>}
    </div>
  );
}

/** Radio dot used by the capture-source and model pickers. */
export function Radio({ on, size, dot, glow }: { on: boolean; size: number; dot: number; glow?: boolean }) {
  return (
    <div
      className="flex flex-none items-center justify-center"
      style={{ width: size, height: size, borderRadius: Math.ceil(size / 2), border: `1.5px solid ${on ? '#ffa24a' : '#4a4640'}` }}
    >
      <div style={{ width: dot, height: dot, borderRadius: Math.ceil(dot / 2), background: on ? '#ffa24a' : 'transparent', boxShadow: on && glow ? '0 0 8px #ffa24a' : 'none' }} />
    </div>
  );
}

export function Glass({ children, className = '', style }: { children: ReactNode; className?: string; style?: CSSProperties }) {
  return (
    <div className={`glass ${className}`} style={style}>
      {children}
    </div>
  );
}
