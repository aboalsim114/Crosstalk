import { ToggleRow, fnt } from '../components/atoms';
import { useApp, type SourceLang } from '../state/store';

const COLS = '140px 110px minmax(0,1fr) 24px';

export function LangPage() {
  const s = useApp();
  return (
    <div className="grid items-start gap-4" style={{ gridTemplateColumns: '310px minmax(0,1fr)' }}>
      <div className="glass flex flex-col gap-[18px] p-5">
        <div className="flex flex-col gap-1.5">
          <span className="lbl">LANGUE CIBLE</span>
          <span style={{ font: fnt(400, 30, 'serif'), color: '#ece6dc' }}>
            Français <span style={{ fontStyle: 'italic', color: '#ffa24a' }}>fr</span>
          </span>
        </div>
        <div className="flex flex-col gap-[9px]">
          <span className="lbl">SOURCES PRÉFÉRÉES</span>
          <div className="flex flex-wrap gap-1.5">
            {(Object.keys(s.src) as SourceLang[]).map((k) => {
              const on = s.src[k];
              return (
                <div
                  key={k}
                  role="checkbox"
                  aria-checked={on}
                  onClick={() => s.toggleSource(k)}
                  className="cursor-pointer rounded-full px-3 py-[5px]"
                  style={{
                    border: `1px solid ${on ? 'rgba(255,162,74,.4)' : 'rgba(255,255,255,.08)'}`,
                    background: on ? 'rgba(255,162,74,.08)' : 'transparent',
                    font: fnt(500, 11.5, 'mono'),
                    color: on ? '#ffc68a' : '#6e6961',
                    transition: 'all .25s',
                  }}
                >
                  {k}
                </div>
              );
            })}
          </div>
          <span style={{ font: fnt(400, 11, 'sans', 1.5), color: '#5a5650' }}>Détection restreinte à ces langues pour les messages courts.</span>
        </div>
        <ToggleRow title="Ignorer le français" sub="Original affiché, aucun appel" on={s.tg.skipFr} onClick={() => s.toggle('skipFr')} />
      </div>

      <div className="glass flex flex-col overflow-hidden">
        <div className="flex items-baseline justify-between px-5 py-4" style={{ borderBottom: '1px solid rgba(255,255,255,.05)' }}>
          <span style={{ font: fnt(400, 22, 'serif'), color: '#ece6dc' }}>Glossaire Counter-Strike</span>
          <span style={{ font: fnt(400, 10.5, 'mono'), color: '#5a5650' }} data-testid="gloss-count">
            {s.gloss.length} termes
          </span>
        </div>
        <div
          className="grid gap-3 px-5 py-[9px]"
          style={{ gridTemplateColumns: COLS, font: fnt(500, 9.5, 'mono'), letterSpacing: '.14em', color: '#5a5650', borderBottom: '1px solid rgba(255,255,255,.04)' }}
        >
          <span>TERME</span>
          <span>TRAITEMENT</span>
          <span>RENDU</span>
          <span />
        </div>
        {s.gloss.map(([t, m, fr], i) => (
          <div key={`${t}-${i}`} className="grid items-center gap-3 px-5 py-2" style={{ gridTemplateColumns: COLS, borderBottom: '1px solid rgba(255,255,255,.035)' }}>
            <span style={{ font: fnt(500, 12.5, 'mono'), color: '#ece6dc' }}>{t}</span>
            <span style={{ font: fnt(400, 11.5), color: m === 'Conserver' ? '#8f8a83' : '#7fd4ff' }}>{m}</span>
            <span style={{ font: fnt(400, 12.5), color: '#8f8a83' }}>{fr}</span>
            <span role="button" aria-label={`Supprimer ${t}`} onClick={() => s.removeTerm(i)} className="del-x text-center" style={{ font: fnt(400, 14, 'mono'), color: '#5a5650' }}>
              ×
            </span>
          </div>
        ))}
        <form
          className="flex gap-2 px-5 py-3"
          onSubmit={(e) => {
            e.preventDefault();
            s.addTerm();
          }}
        >
          <input className="text-input flex-1" value={s.newTerm} onChange={(e) => s.set({ newTerm: e.target.value })} placeholder="Ajouter un terme (ex. « boost »)" />
          <button type="submit" className="pill pill-ember px-4 py-2" style={{ font: fnt(500, 12.5) }}>
            Ajouter
          </button>
        </form>
      </div>
    </div>
  );
}
