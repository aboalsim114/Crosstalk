import { config } from '../config';
import { Radio, Seg, Slider, Toggle, ToggleRow, fnt } from '../components/atoms';
import { MODELS } from '../state/data';
import { useApp, type Accel, type CaptureSource, type Engine } from '../state/store';

const ACCELS: [Accel, string][] = [
  ['cpu', 'CPU'],
  ['cuda', 'CUDA'],
];
const ENGINES: [Engine, string][] = [
  ['DeepL', 'DeepL'],
  ['Azure', 'Azure'],
  ['Local', 'Local (bêta)'],
];

function CaptureCard() {
  const s = useApp();
  const fb = config.win10Fallback;
  const opts: [CaptureSource, string, string, string, boolean][] = [
    ['process', 'Audio de CS2 uniquement', 'Process loopback WASAPI ciblant cs2.exe. Sans injection.', fb ? 'INDISPONIBLE' : 'RECOMMANDÉ', fb],
    ['device', 'Périphérique de sortie', 'Loopback du casque — capte aussi Discord, navigateur…', 'REPLI', false],
    ['wav', 'Fichier WAV de démonstration', 'Tester la chaîne sans lancer CS2', 'TEST', false],
  ];
  const testing = s.test > 0 && s.test < 100;
  return (
    <div className="glass flex flex-col gap-2.5 p-5">
      <div className="lbl">SOURCE DE CAPTURE</div>
      {opts.map(([id, name, sub, tag, disabled]) => {
        const on = s.capture === id && !disabled;
        return (
          <div
            key={id}
            role="radio"
            aria-checked={on}
            aria-disabled={disabled}
            onClick={() => !disabled && s.set({ capture: id })}
            className="flex gap-3 rounded-xl px-3.5 py-[13px]"
            style={{
              border: `1px solid ${on ? 'rgba(255,162,74,.35)' : 'rgba(255,255,255,.06)'}`,
              background: on ? 'rgba(255,162,74,.06)' : 'transparent',
              cursor: disabled ? 'not-allowed' : 'pointer',
              opacity: disabled ? 0.45 : 1,
              transition: 'all .3s',
            }}
          >
            <div className="mt-0.5">
              <Radio on={on} size={15} dot={6} glow />
            </div>
            <div className="flex flex-1 flex-col gap-[3px]">
              <div className="flex justify-between gap-2">
                <span className="row-title">{name}</span>
                <span style={{ font: fnt(500, 9.5, 'mono'), letterSpacing: '.12em', color: tag === 'RECOMMANDÉ' ? '#7fe0a8' : tag === 'INDISPONIBLE' ? '#ff8a7a' : '#6e6961' }}>{tag}</span>
              </div>
              <span style={{ font: fnt(400, 11.5, 'sans', 1.45), color: '#6e6961' }}>{sub}</span>
            </div>
          </div>
        );
      })}
      {(s.capture === 'device' || fb) && (
        <div className="warn-box">Le loopback système capte tout le son du périphérique — y compris des conversations privées d’autres applications.</div>
      )}
      <div className="flex items-center gap-3.5 pt-1.5">
        <div className="pill pill-ember flex-none px-4 py-[9px]" role="button" onClick={s.runTest} style={{ font: fnt(500, 12.5) }}>
          {testing ? 'Écoute…' : 'Test audio 10 s'}
        </div>
        <div className="flex flex-1 flex-col gap-1.5">
          <div className="h-0.5 overflow-hidden rounded-[1px]" style={{ background: 'rgba(255,255,255,.07)' }}>
            <div className="h-0.5" data-testid="test-progress" style={{ width: `${s.test}%`, background: '#ffa24a', boxShadow: '0 0 10px #ffa24a', transition: 'width .25s linear' }} />
          </div>
          <span style={{ font: fnt(400, 11), color: '#6e6961' }}>
            {s.testDone
              ? 'Voix détectée · ~1,3 s de traitement par segment de 4 s'
              : s.test > 0
                ? `${Math.round(s.test / 10)} / 10 s`
                : 'Valide l’entrée, la détection de voix et le temps de traitement'}
          </span>
        </div>
      </div>
    </div>
  );
}

function VadCard() {
  const vad = useApp((s) => s.vad);
  const noise = useApp((s) => s.tg.noise);
  const { set, toggle } = useApp.getState();
  return (
    <div className="glass flex flex-col gap-3.5 p-5">
      <div className="lbl">DÉTECTION DE PAROLE · VAD</div>
      <Slider label="Seuil de détection" unit="%" min={20} max={90} value={vad} onChange={(v) => set({ vad: v })} hint="Plus haut = ignore davantage les tirs et bruitages" />
      <div className="flex items-center justify-between gap-3">
        <div className="flex flex-col gap-[3px]">
          <span className="row-title">
            Suppression de bruit <span style={{ font: fnt(500, 9, 'mono'), letterSpacing: '.12em', color: '#ffa24a', marginLeft: 6 }}>EXPÉRIMENTAL</span>
          </span>
          <span className="row-sub">Pré-roll 250 ms · post-roll 400 ms</span>
        </div>
        <Toggle on={noise} onClick={() => toggle('noise')} label="Suppression de bruit" />
      </div>
    </div>
  );
}

function ModelCard() {
  const s = useApp();
  return (
    <div className="glass flex flex-col gap-2 p-5">
      <div className="flex items-center justify-between pb-1">
        <span className="lbl">MODÈLE LOCAL · WHISPER</span>
        <Seg options={ACCELS} value={s.accel} onChange={(v) => s.set({ accel: v })} item={{ padding: '3px 11px', font: fnt(500, 10.5, 'mono') }} />
      </div>
      {MODELS.map(([id, size, desc, installed]) => {
        const on = s.model === id;
        const d = s.dl[id];
        const ok = installed || d === 100;
        const st = on ? 'Actif' : ok ? 'Installé' : d ? `${d} %` : 'Télécharger';
        return (
          <div
            key={id}
            role="radio"
            aria-checked={on}
            onClick={() => s.pickModel(id, installed)}
            className="grid cursor-pointer items-center gap-3 rounded-[10px] px-3 py-2.5"
            style={{
              gridTemplateColumns: '15px minmax(0,1fr) auto',
              border: `1px solid ${on ? 'rgba(255,162,74,.35)' : 'rgba(255,255,255,.05)'}`,
              background: on ? 'rgba(255,162,74,.06)' : 'transparent',
              transition: 'all .3s',
            }}
          >
            <Radio on={on} size={13} dot={5} />
            <div className="flex min-w-0 flex-col gap-0.5">
              <span style={{ font: fnt(500, 12.5, 'mono'), color: '#ece6dc' }}>whisper-{id}</span>
              <span style={{ font: fnt(400, 11), color: '#6e6961' }}>{desc}</span>
            </div>
            <div className="flex flex-col items-end gap-0.5">
              <span style={{ font: fnt(400, 11, 'mono'), color: '#8f8a83' }}>{size}</span>
              <span style={{ font: fnt(500, 10.5), color: on ? '#ffa24a' : ok ? '#7fe0a8' : '#7fd4ff' }}>{st}</span>
            </div>
          </div>
        );
      })}
      <div className="pt-1" style={{ font: fnt(400, 10.5, 'sans', 1.5), color: '#5a5650' }}>
        whisper.cpp via whisper-rs · licence MIT · %LOCALAPPDATA%\Crosstalk\models
      </div>
    </div>
  );
}

function EngineCard() {
  const engine = useApp((s) => s.engine);
  const cloud = useApp((s) => s.tg.cloud);
  const { set, toggle } = useApp.getState();
  return (
    <div className="glass flex flex-col gap-3 p-5">
      <div className="lbl">MOTEUR DE TRADUCTION</div>
      <Seg options={ENGINES} value={engine} onChange={(v) => set({ engine: v })} grid={3} item={{ padding: '6px 0', textAlign: 'center', font: fnt(500, 11.5) }} />
      <div style={{ font: fnt(400, 11, 'sans', 1.5), color: '#6e6961' }}>Clé API dans le Gestionnaire d’identification Windows — jamais dans config.json ni dans les journaux.</div>
      <ToggleRow title="Transcription cloud (opt-in)" sub="Pour CPU/GPU insuffisants" on={cloud} onClick={() => toggle('cloud')} />
      {cloud && <div className="warn-box">Les segments vocaux détectés seront envoyés au fournisseur choisi. Coût estimé : ~0,36 € / heure de parole.</div>}
    </div>
  );
}

export function AudioPage() {
  return (
    <div className="grid grid-cols-2 items-start gap-4">
      <div className="flex flex-col gap-4">
        <CaptureCard />
        <VadCard />
      </div>
      <div className="flex flex-col gap-4">
        <ModelCard />
        <EngineCard />
      </div>
    </div>
  );
}
