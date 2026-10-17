import { fnt } from '../../shared/ui/atoms';
import { useT } from '../../shared/i18n';
import { isSpeechLang, NATIVE_NAME, PLAYER_LANGS, SPEECH_LANGS, TARGET_LANGS, type TargetLang } from './langs';
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

/** The user's language (one); the players' languages are detected, only listed. */
export function LanguagePicker() {
  const t = useT();
  const target = useApp((s) => s.target);
  const setTarget = (l: TargetLang) => useApp.setState({ target: l });
  return (
    <div className="flex flex-col gap-4" data-testid="language-picker">
      <div className="flex flex-col gap-2">
        <span className="lbl">{t('myLang.me')}</span>
        <div role="radiogroup" aria-label={t('myLang.me')} className="flex flex-wrap gap-1.5">
          {TARGET_LANGS.map((l) => (
            <Chip key={l} role="radio" on={target === l} onClick={() => setTarget(l)}>
              <span dir="auto">{NATIVE_NAME[l]}</span>
            </Chip>
          ))}
        </div>
      </div>
      <div className="flex flex-col gap-2" data-testid="detected-languages">
        <span className="lbl">{t('myLang.players')}</span>
        <div className="flex flex-col gap-1">
          {[
            [SPEECH_LANGS.filter((l) => l !== target), t('myLang.voice')] as const,
            [PLAYER_LANGS.filter((l) => !isSpeechLang(l) && l !== target), t('myLang.chatOnly')] as const,
          ].map(([langs, how]) => (
            <span key={how} style={{ font: fnt(400, 11.5, 'sans', 1.5), color: '#8f8a83' }}>
              <span dir="auto">{langs.map((l) => NATIVE_NAME[l]).join(' · ')}</span> <span style={{ color: '#5a5650' }}>— {how}</span>
            </span>
          ))}
        </div>
        <span style={{ font: fnt(400, 10.5, 'sans', 1.5), color: '#5a5650' }}>{t('myLang.playersHint')}</span>
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
