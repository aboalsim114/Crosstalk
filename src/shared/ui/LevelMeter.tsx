import { formatDb, levelToUnit } from '../../features/audio/audio';
import type { LevelReading } from '../ipc';
import { fnt } from './atoms';

/** Real input level from the Rust capture: RMS bar plus peak marker, in dBFS. */
export function LevelMeter({ level, active, color = '#7fe0a8' }: { level: LevelReading | null; active: boolean; color?: string }) {
  const rms = active && level ? levelToUnit(level.rmsDb) : 0;
  const peak = active && level ? levelToUnit(level.peakDb) : 0;
  return (
    <div className="flex flex-col gap-1.5" data-testid="level-meter">
      <div className="flex justify-between">
        <span className="lbl">NIVEAU D’ENTRÉE · RÉEL</span>
        <span style={{ font: fnt(500, 10.5, 'mono'), color: active ? '#bdb6ab' : '#4a4640' }}>
          {active && level ? `${formatDb(level.rmsDb)} rms · ${formatDb(level.peakDb)} crête` : 'aucune capture'}
        </span>
      </div>
      <div className="relative h-1.5 overflow-hidden rounded-full" style={{ background: 'rgba(255,255,255,.06)' }} role="meter" aria-label="Niveau audio" aria-valuemin={0} aria-valuemax={1} aria-valuenow={Number(rms.toFixed(2))}>
        <div className="absolute inset-y-0 left-0 rounded-full" style={{ width: `${rms * 100}%`, background: color, boxShadow: `0 0 10px ${color}`, transition: 'width .08s linear' }} />
        <div className="absolute inset-y-0 w-0.5" style={{ left: `calc(${peak * 100}% - 1px)`, background: '#ece6dc', opacity: peak > 0 ? 0.9 : 0, transition: 'left .08s linear' }} />
      </div>
      <div className="flex justify-between" style={{ font: fnt(400, 9.5, 'mono'), color: '#4a4640' }}>
        <span>−60</span>
        <span>−40</span>
        <span>−20</span>
        <span>0 dBFS</span>
      </div>
    </div>
  );
}
