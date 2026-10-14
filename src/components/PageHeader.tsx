import { useT, type MessageKey } from '../i18n';
import { NAV } from '../state/data';
import { isRunning, useApp, type Session } from '../state/store';
import { fnt } from './atoms';

export const SESSION_BADGE: Record<Session, [MessageKey, string]> = {
  IDLE: ['session.idle', '#8f8a83'],
  INITIALIZING: ['session.initializing', '#7fd4ff'],
  LISTENING: ['session.listening', '#7fe0a8'],
  DEGRADED: ['session.degraded', '#ffa24a'],
  PAUSED: ['session.paused', '#ffa24a'],
  STOPPING: ['session.stopping', '#8f8a83'],
  STOPPED: ['session.stopped', '#8f8a83'],
};

export function PageHeader() {
  const page = useApp((s) => s.page);
  const session = useApp((s) => s.session);
  const paused = useApp((s) => s.paused);
  const { togglePause, emergency, toggleSession } = useApp.getState();
  const t = useT();
  const running = isRunning({ session });
  const pi = Math.max(0, NAV.findIndex(([id]) => id === page));
  const [badge, badgeColor] = SESSION_BADGE[session];

  return (
    <div className="flex h-24 flex-none items-end justify-between px-8 pb-[18px]">
      <div className="flex flex-col gap-1.5">
        <div style={{ font: fnt(500, 10, 'mono'), letterSpacing: '.2em', color: '#6e6961' }}>
          {t('header.chapter', { n: String(pi + 1).padStart(2, '0'), total: String(NAV.length).padStart(2, '0') })}
        </div>
        <h1 style={{ font: fnt(400, 36, 'serif', 1), color: '#ece6dc', letterSpacing: '-.01em' }}>{t(`nav.${NAV[pi][0]}`)}</h1>
      </div>
      <div className="flex items-center gap-2.5">
        <div
          data-testid="session-badge"
          className="flex items-center gap-[9px] rounded-full px-3.5 py-2"
          style={{ border: '1px solid rgba(255,255,255,.07)', background: 'rgba(10,10,12,.5)', font: fnt(500, 10.5, 'mono'), letterSpacing: '.14em', color: badgeColor }}
        >
          <div className="h-1.5 w-1.5 rounded-[3px]" style={{ background: badgeColor, boxShadow: `0 0 10px ${badgeColor}` }} />
          {t(badge)}
        </div>
        <div className="pill pill-ghost px-4 py-2.5" role="button" onClick={togglePause} style={{ font: fnt(500, 12.5), color: running ? '#ece6dc' : '#4a4640' }}>
          {paused ? t('action.resume') : t('action.pause')}
        </div>
        <div className="pill pill-danger px-4 py-2.5" role="button" onClick={emergency} style={{ font: fnt(500, 12.5) }}>
          {t('action.emergency')}
        </div>
        <div
          role="button"
          onClick={toggleSession}
          className="min-w-[120px] cursor-pointer rounded-full px-[22px] py-2.5 text-center"
          style={{
            background: running ? 'rgba(255,255,255,.08)' : '#ffa24a',
            boxShadow: running ? 'none' : '0 0 30px rgba(255,162,74,.4)',
            font: fnt(600, 12.5),
            color: running ? '#ece6dc' : '#120c06',
            transition: 'all .4s',
          }}
        >
          {running ? t('action.stop') : t('action.start')}
        </div>
      </div>
    </div>
  );
}
