import { windowControls } from '../../shared/ipc';
import { useT } from '../../shared/i18n';
import { isRunning, useApp } from '../state/store';
import { fnt } from '../../shared/ui/atoms';

/** HH:MM:SS:FF session timecode at 25 fps, frozen at zero when no session runs. */
export function timecode(startedAt: number, now: number, running: boolean): string {
  if (!startedAt || !running) return '00:00:00:00';
  const d = Math.max(0, now - startedAt);
  const z = (n: number) => String(n).padStart(2, '0');
  return `${z(Math.floor(d / 3600000))}:${z(Math.floor(d / 60000) % 60)}:${z(Math.floor(d / 1000) % 60)}:${z(Math.floor(d / 40) % 25)}`;
}

export function TitleBar() {
  const tc = useApp((s) => timecode(s.startedAt, s.now, isRunning(s)));
  const t = useT();
  return (
    <div
      data-tauri-drag-region
      className="relative z-[6] flex h-9 flex-none items-center justify-between pl-4"
      style={{ borderBottom: '1px solid rgba(255,255,255,.04)' }}
    >
      <div data-tauri-drag-region className="flex items-center gap-3" style={{ font: fnt(500, 10.5, 'mono'), letterSpacing: '.14em', color: '#6e6961' }}>
        <div className="h-[7px] w-[7px] rounded-[4px]" style={{ background: '#ffa24a', boxShadow: '0 0 10px #ffa24a' }} />
        <span data-tauri-drag-region>CROSSTALK</span>
        <span data-tauri-drag-region style={{ color: '#3d3a36' }}>
          —
        </span>
        <span data-tauri-drag-region data-testid="timecode" style={{ color: '#8f8a83' }}>
          {tc}
        </span>
      </div>
      <div className="flex h-9" style={{ font: fnt(400, 12, 'mono', '36px'), color: '#6e6961' }}>
        <div className="win-btn" role="button" aria-label={t('window.minimize')} onClick={() => windowControls.minimize()}>
          —
        </div>
        <div className="win-btn" role="button" aria-label={t('window.maximize')} onClick={() => windowControls.toggleMaximize()}>
          ▢
        </div>
        <div className="win-btn close" role="button" aria-label={t('window.close')} onClick={() => windowControls.close()}>
          ✕
        </div>
      </div>
    </div>
  );
}
