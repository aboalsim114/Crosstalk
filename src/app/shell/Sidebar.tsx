import { useT, type MessageKey } from '../../shared/i18n';
import { NAV } from '../state/data';
import { useApp } from '../state/store';
import { fnt } from '../../shared/ui/atoms';

const SHORTCUTS: [MessageKey, string, string][] = [
  ['action.overlay', '⌃⌥F8', '#bdb6ab'],
  ['action.pause', '⌃⌥F9', '#bdb6ab'],
  ['action.emergency', '⌃⌥F10', '#ff8a7a'],
];

export function Sidebar() {
  const page = useApp((s) => s.page);
  const setPage = useApp((s) => s.setPage);
  const t = useT();
  return (
    <nav
      className="flex w-[232px] flex-none flex-col px-[18px] pt-7 pb-[18px]"
      style={{ borderRight: '1px solid rgba(255,255,255,.05)', background: 'linear-gradient(90deg,rgba(4,4,5,.75),rgba(4,4,5,.35))' }}
    >
      <div className="flex flex-col gap-1.5 px-2 pb-[34px]">
        <div style={{ font: fnt(400, 30, 'serif', 0.95), color: '#ece6dc', letterSpacing: '-.01em' }}>
          Cross
          <br />
          <span style={{ fontStyle: 'italic', color: '#ffa24a' }}>talk</span>
        </div>
        <div style={{ font: fnt(500, 9.5, 'mono'), letterSpacing: '.18em', color: '#5a5650' }}>{t('brand.tagline')}</div>
      </div>
      <div className="flex flex-col gap-0.5">
        {NAV.map(([id], i) => {
          const on = page === id;
          return (
            <div key={id} className="nav-item" aria-current={on ? 'page' : undefined} onClick={() => setPage(id)} style={{ background: on ? 'rgba(255,255,255,.045)' : 'transparent' }}>
              <span className="w-[18px]" style={{ font: fnt(500, 10, 'mono'), color: on ? '#ffa24a' : '#4a4640', transition: 'color .3s' }}>
                {String(i + 1).padStart(2, '0')}
              </span>
              <span className="flex-1" style={{ font: fnt(400, 13.5), color: on ? '#ece6dc' : '#8f8a83', transition: 'color .3s' }}>
                {t(`nav.${id}`)}
              </span>
              <div className="h-1 w-1 rounded-sm" style={{ background: '#ffa24a', opacity: on ? 1 : 0, boxShadow: '0 0 8px #ffa24a' }} />
            </div>
          );
        })}
      </div>
      <div className="flex-1" />
      <div className="flex flex-col gap-[9px] px-2.5 pt-4 pb-1" style={{ borderTop: '1px solid rgba(255,255,255,.05)' }}>
        {SHORTCUTS.map(([label, keys, color]) => (
          <div key={label} className="flex justify-between" style={{ font: fnt(400, 11.5), color: '#8f8a83' }}>
            <span>{t(label)}</span>
            <span style={{ font: fnt(500, 10, 'mono'), color }}>{keys}</span>
          </div>
        ))}
      </div>
    </nav>
  );
}
