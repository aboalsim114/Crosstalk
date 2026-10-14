import { fnt } from '../../shared/ui/atoms';
import { detectLocale, LOCALES, useT, type UiLang } from '../../shared/i18n';
import { useApp } from '../../app/state/store';

const nameOf = (id: string) => LOCALES.find((l) => l.id === id)?.name ?? id;

/** Interface language: automatic (Windows) or one of the supported locales. */
export function UiLangCard() {
  const uiLang = useApp((s) => s.uiLang);
  const set = useApp((s) => s.set);
  const t = useT();
  const system = detectLocale(typeof navigator === 'undefined' ? [] : navigator.languages ?? [navigator.language]);
  const options: { id: UiLang; label: string }[] = [{ id: 'auto', label: t('uiLang.auto', { name: nameOf(system) }) }, ...LOCALES.map((l) => ({ id: l.id as UiLang, label: l.name }))];
  return (
    <div className="glass flex flex-col gap-3 p-5" data-testid="ui-lang-card">
      <span className="lbl">{t('uiLang.title')}</span>
      <div role="radiogroup" aria-label={t('uiLang.label')} className="flex flex-wrap gap-1.5">
        {options.map((o) => {
          const on = uiLang === o.id;
          return (
            <div
              key={o.id}
              role="radio"
              aria-checked={on}
              lang={o.id === 'auto' ? undefined : o.id}
              onClick={() => set({ uiLang: o.id })}
              className="cursor-pointer rounded-full px-3 py-[5px]"
              style={{
                border: `1px solid ${on ? 'rgba(255,162,74,.4)' : 'rgba(255,255,255,.08)'}`,
                background: on ? 'rgba(255,162,74,.08)' : 'transparent',
                font: fnt(500, 11.5),
                color: on ? '#ffc68a' : '#8f8a83',
                transition: 'all .25s',
              }}
            >
              {o.label}
            </div>
          );
        })}
      </div>
      <span style={{ font: fnt(400, 11, 'sans', 1.5), color: '#5a5650' }}>{t('uiLang.note')}</span>
    </div>
  );
}
