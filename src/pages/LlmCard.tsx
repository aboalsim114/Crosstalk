import { useState } from 'react';
import { translation as nativeTranslation, type EngineChoice, type LlmBackend } from '../backend';
import { fnt, Seg } from '../components/atoms';
import { startSetup, useSetup } from '../state/setup';
import { useApp } from '../state/store';
import { llmView, rescanTranslation, useTranslation } from '../state/translation';

/** TranslateGemma 4B: state, size, backend, measured time, download / repair. */
export function LlmCard() {
  const st = useTranslation((s) => s.status?.llm);
  const dl = useSetup((s) => s.status);
  const engine = useApp((s) => s.trEngine);
  const backend = useApp((s) => s.llmBackend);
  const set = useApp.setState;
  const [check, setCheck] = useState<{ busy: boolean; msg: string | null; ok: boolean }>({ busy: false, msg: null, ok: true });
  const v = llmView(st, dl);
  const verify = async () => {
    setCheck({ busy: true, msg: 'Lecture complète du fichier (environ 2,5 Go)…', ok: true });
    try {
      const m = await nativeTranslation.verifyLlm();
      setCheck({ busy: false, msg: m ? `Intact · SHA-256 ${m.sha256.slice(0, 12)}…` : null, ok: true });
    } catch (e) {
      setCheck({ busy: false, msg: String(e), ok: false });
    }
    await rescanTranslation();
  };
  return (
    <div className="glass flex flex-col gap-3 p-5" data-testid="llm-card">
      <div className="flex items-center justify-between">
        <span className="lbl">MOTEUR DE TRADUCTION · {st ? `${st.name.toUpperCase()}` : 'TRANSLATEGEMMA 4B'}</span>
        <span data-testid="llm-state" style={{ font: fnt(500, 9.5, 'mono'), letterSpacing: '.12em', color: v.color }}>
          {v.state}
        </span>
      </div>
      <div style={{ font: fnt(400, 11, 'sans', 1.5), color: '#8f8a83' }}>
        Modèle de traduction de Google exécuté sur ce PC (llama.cpp). Les annonces connues du lexique restent traduites instantanément ; le reste passe par TranslateGemma. Aucune clé API, rien n’est envoyé en ligne.
      </div>
      <div role="status" style={{ font: fnt(400, 11.5, 'sans', 1.5), color: v.color === '#ff8a7a' ? '#ff8a7a' : '#ece6dc' }} data-testid="llm-detail">
        {v.detail}
      </div>
      {v.progress && (
        <div className="flex flex-col gap-1.5">
          <div className="h-1 overflow-hidden rounded-full" style={{ background: 'rgba(255,255,255,.08)' }}>
            <div data-testid="llm-progress" style={{ width: `${v.progress.pct}%`, height: '100%', background: '#ffa24a', transition: 'width .4s' }} />
          </div>
          <span style={{ font: fnt(400, 10.5, 'mono'), color: '#8f8a83' }}>{v.progress.text}</span>
        </div>
      )}
      {st && (
        <div className="grid grid-cols-[auto_1fr] gap-x-4 gap-y-1" style={{ font: fnt(400, 11.5) }}>
          <span style={{ color: '#6e6961' }}>Modèle</span>
          <span className="truncate text-right" style={{ color: '#ece6dc' }} title={`${st.repo}/${st.file}`}>
            {st.name} · {st.quant} · {(st.sizeMb / 1000).toFixed(2).replace('.', ',')} Go
          </span>
          <span style={{ color: '#6e6961' }}>Exécution</span>
          <span className="truncate text-right" style={{ color: '#ece6dc' }} data-testid="llm-backend">
            {v.backend ?? '—'}
          </span>
          <span style={{ color: '#6e6961' }}>Temps mesuré</span>
          <span className="text-right" style={{ color: '#ece6dc' }} data-testid="llm-timing">
            {v.timing ?? '—'}
          </span>
        </div>
      )}
      {st && (
        <div className="flex flex-col gap-2">
          <div className="flex items-center justify-between gap-3">
            <span style={{ font: fnt(500, 12), color: '#ece6dc' }}>Moteur utilisé</span>
            <Seg<EngineChoice>
              options={[
                ['auto', 'Auto'],
                ['translategemma', 'TranslateGemma'],
                ['opus', 'Opus-MT'],
              ]}
              value={engine}
              onChange={(e) => set({ trEngine: e })}
              item={{ padding: '4px 10px', font: fnt(500, 11) }}
            />
          </div>
          <div className="flex items-center justify-between gap-3">
            <span style={{ font: fnt(500, 12), color: '#ece6dc' }}>Calcul</span>
            <Seg<LlmBackend>
              options={[
                ['auto', 'Auto'],
                ['cpu', 'Processeur'],
                ['gpu', 'Carte graphique'],
              ]}
              value={backend}
              onChange={(b) => set({ llmBackend: b })}
              item={{ padding: '4px 10px', font: fnt(500, 11) }}
            />
          </div>
          <span style={{ font: fnt(400, 10.5, 'sans', 1.5), color: '#5a5650' }}>
            Auto : la carte graphique seulement s’il reste assez de mémoire vidéo à côté de CS2 et de Whisper ; TranslateGemma s’efface devant Opus-MT s’il est trop lent ou s’il échoue.
          </span>
        </div>
      )}
      {st && (
        <div className="flex flex-wrap items-center gap-2">
          {v.canDownload && (
            <div className="pill pill-ember px-3.5 py-[7px]" role="button" onClick={() => startSetup(useApp.getState())} style={{ font: fnt(500, 11.5) }}>
              {st.install.state === 'partial' ? 'Reprendre le téléchargement' : 'Télécharger'}
            </div>
          )}
          {v.canVerify && (
            <div className="pill pill-ghost px-3.5 py-[7px]" role="button" aria-disabled={check.busy} onClick={() => !check.busy && verify()} style={{ font: fnt(500, 11.5) }}>
              {check.busy ? 'Vérification…' : 'Vérifier / réparer'}
            </div>
          )}
          <div className="pill pill-ghost px-3.5 py-[7px]" role="button" onClick={() => rescanTranslation()} style={{ font: fnt(500, 11.5) }}>
            Réessayer
          </div>
        </div>
      )}
      {check.msg && (
        <span data-testid="llm-check" style={{ font: fnt(400, 11, 'sans', 1.5), color: check.ok ? '#7fe0a8' : '#ff8a7a' }}>
          {check.msg}
        </span>
      )}
      {st && (
        <span style={{ font: fnt(400, 10.5, 'sans', 1.5), color: '#5a5650' }}>
          {st.baseModel} · conversion GGUF {st.repo.split('/')[0]} · {st.license} ({st.licenseUrl}) · %LOCALAPPDATA%\Crosstalk\models
        </span>
      )}
    </div>
  );
}
