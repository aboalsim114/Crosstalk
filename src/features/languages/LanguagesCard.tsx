import { fnt } from '../../shared/ui/atoms';
import { useT } from '../../shared/i18n';
import { NATIVE_NAME, PLAYER_LANGS, TARGET_LANGS, translatedSources, type PlayerLang, type TargetLang } from './langs';
import { useApp } from '../../app/state/store';

function Chip({ on, disabled, onClick, children, role }: { on: boolean; disabled?: boolean; onClick: () => void; children: React.ReactNode; role: 'radio' | 'checkbox' }) {
  return (
    <div
      role={role}
      aria-checked={on}
      aria-disabled={disabled}
      onClick={() => !disabled && onClick()}
      className={`rounded-full px-3 py-[5px] ${disabled ? '' : 'cursor-pointer'}`}
      style={{
        border: `1px solid ${on ? 'rgba(255,162,74,.45)' : 'rgba(255,255,255,.08)'}`,
        background: on ? 'rgba(255,162,74,.1)' : 'transparent',
        font: fnt(500, 12),
        color: disabled ? '#5a5650' : on ? '#ffc68a' : '#8f8a83',
        transition: 'all .25s',
      }}
    >
      {children}
    </div>
  );
}

/** The user's language (one) and the players' languages (several). */
export function LanguagePicker() {
  const t = useT();
  const target = useApp((s) => s.target);
  const src = useApp((s) => s.src);
  const set = useApp.setState;
  const routes = translatedSources(target, src);
  const setTarget = (l: TargetLang) => set({ target: l });
  const toggle = (l: PlayerLang) => set((s) => ({ src: { ...s.src, [l]: !s.src[l] } }));
  return (
    <div className="flex flex-col gap-4" data-testid="language-picker">
      <div className="flex flex-col gap-2">
        <span className="lbl">{t('myLang.me')}</span>
        <div role="radiogroup" aria-label={t('myLang.me')} className="flex flex-wrap gap-1.5">
          {TARGET_LANGS.map((l) => (
            <Chip key={l} role="radio" on={target === l} onClick={() => setTarget(l)}>
              {NATIVE_NAME[l]}
            </Chip>
          ))}
        </div>
      </div>
      <div className="flex flex-col gap-2">
        <span className="lbl">{t('myLang.players')}</span>
        <div role="group" aria-label={t('myLang.players')} className="flex flex-wrap gap-1.5">
          {PLAYER_LANGS.map((l) => (
            <Chip key={l} role="checkbox" on={l !== target && !!src[l]} disabled={l === target} onClick={() => toggle(l)}>
              {NATIVE_NAME[l]}
            </Chip>
          ))}
        </div>
        <span style={{ font: fnt(400, 10.5, 'sans', 1.5), color: '#5a5650' }}>{t('myLang.playersHint')}</span>
      </div>
      <div className="flex flex-col gap-1" data-testid="language-routes">
        {routes.length === 0 ? (
          <span style={{ font: fnt(400, 11.5, 'sans', 1.5), color: '#ffa24a' }}>{t('myLang.none')}</span>
        ) : (
          routes.map((s) => (
            <span key={s} style={{ font: fnt(400, 11.5, 'mono'), color: '#8f8a83' }}>
              {s.toUpperCase()} → {target.toUpperCase()}
            </span>
          ))
        )}
      </div>
      <span style={{ font: fnt(400, 10.5, 'sans', 1.5), color: '#5a5650' }}>{t('myLang.llmHint')}</span>
    </div>
  );
}

/** Langues › Mes langues. */
export function MyLanguagesCard() {
  const t = useT();
  return (
    <div className="glass flex flex-col gap-4 p-5" data-testid="my-languages">
      <span className="lbl">{t('myLang.title')}</span>
      <LanguagePicker />
    </div>
  );
}

/** First launch: the user's languages before anything is downloaded. */
export function LanguageSetup() {
  const t = useT();
  const done = useApp((s) => s.langSetupDone);
  if (done) return null;
  return (
    <div className="absolute inset-0 z-[50] flex items-center justify-center" style={{ background: 'rgba(8,7,6,.78)', backdropFilter: 'blur(6px)' }} role="dialog" aria-modal="true" aria-label={t('setupLang.title')}>
      <div className="glass flex w-[560px] max-w-[92%] flex-col gap-5 p-7" style={{ background: 'rgba(18,16,14,.94)' }}>
        <div className="flex flex-col gap-2">
          <span style={{ font: fnt(400, 30, 'serif'), color: '#ece6dc' }}>{t('setupLang.title')}</span>
          <span style={{ font: fnt(400, 12.5, 'sans', 1.55), color: '#8f8a83' }}>{t('setupLang.lead')}</span>
        </div>
        <LanguagePicker />
        <div className="flex justify-end">
          <div className="pill pill-ember px-5 py-2" role="button" onClick={() => useApp.setState({ langSetupDone: true })} style={{ font: fnt(500, 13) }}>
            {t('setupLang.continue')}
          </div>
        </div>
      </div>
    </div>
  );
}
