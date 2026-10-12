import { config } from '../config';
import { useState } from 'react';
import { translation as nativeTranslation, type Translation } from '../backend';
import { Radio, Seg, Slider, Toggle, ToggleRow, fnt } from '../components/atoms';
import { LevelMeter } from '../components/LevelMeter';
import { captureSummary, cs2Summary, isCapturing, refreshDevices, runCaptureTest, testMessage, useAudio } from '../state/audio';
import { MODELS, type ModelId } from '../state/data';
import { engineSummary, formatMs, rescanModels, statsLine, useSpeech } from '../state/speech';
import { useApp, type Accel, type CaptureSource } from '../state/store';
import { pairSummary, percentile, rescanTranslation, useTranslation } from '../state/translation';

const ACCELS: [Accel, string][] = [
  ['cpu', 'CPU'],
  ['cuda', 'CUDA'],
];

function CaptureCard() {
  const s = useApp();
  const audio = useAudio();
  const native = audio.native;
  const fb = !native && config.win10Fallback;
  const cs2 = cs2Summary(audio.cs2);
  const opts: [CaptureSource, string, string, string, boolean][] = native
    ? [
        ['process', 'Audio de CS2 uniquement', `Process loopback WASAPI ciblant cs2.exe. Sans injection. ${audio.cs2?.running ? `CS2 détecté (PID ${audio.cs2.pid}).` : 'CS2 non détecté.'}`, 'RECOMMANDÉ', false],
        ['device', 'Périphérique de sortie', 'Loopback du casque — capte aussi Discord, navigateur…', 'REPLI', false],
        ['wav', 'Fichier WAV de démonstration', 'Hors application : crosstalk-asr-bench --manifest', 'OUTIL', true],
      ]
    : [
        ['process', 'Audio de CS2 uniquement', 'Process loopback WASAPI ciblant cs2.exe. Sans injection.', fb ? 'INDISPONIBLE' : 'RECOMMANDÉ', fb],
        ['device', 'Périphérique de sortie', 'Loopback du casque — capte aussi Discord, navigateur…', 'REPLI', false],
        ['wav', 'Fichier WAV de démonstration', 'Tester la chaîne sans lancer CS2', 'TEST', false],
      ];
  const capture = captureSummary(audio.status);
  const capturing = isCapturing(audio.status);
  const testing = native ? audio.test.running : s.test > 0 && s.test < 100;
  const progress = native ? audio.test.progress : s.test;
  const deviceMode = s.capture === 'device';
  return (
    <div className="glass flex flex-col gap-2.5 p-5">
      <div className="flex items-center justify-between">
        <span className="lbl">SOURCE DE CAPTURE</span>
        {native && <span style={{ font: fnt(500, 9.5, 'mono'), letterSpacing: '.12em', color: cs2.color }}>CS2 · {cs2.value.toUpperCase()}</span>}
      </div>
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
      {native && (deviceMode || s.capture === 'process') && (
        <label className="flex items-center justify-between gap-3" style={{ font: fnt(400, 12), color: '#bdb6ab' }}>
          <span>{deviceMode ? 'Périphérique capturé' : 'Périphérique de repli'}</span>
          <select
            aria-label="Périphérique de sortie"
            value={audio.deviceId ?? ''}
            onChange={(e) => useAudio.setState({ deviceId: e.target.value || null })}
            onFocus={() => refreshDevices()}
            className="text-input max-w-[230px] cursor-pointer truncate"
            style={{ font: fnt(400, 11.5), padding: '6px 12px' }}
          >
            <option value="">Périphérique par défaut de Windows</option>
            {audio.devices.map((d) => (
              <option key={d.id} value={d.id}>
                {d.name}
                {d.isDefault ? ' (par défaut)' : ''}
              </option>
            ))}
          </select>
        </label>
      )}
      {(deviceMode || fb || (native && audio.status.fallbackReason)) && (
        <div className="warn-box">Le loopback système capte tout le son du périphérique — y compris des conversations privées d’autres applications.</div>
      )}
      {native && (
        <div className="flex flex-col gap-2 rounded-xl px-3.5 py-3" style={{ border: '1px solid rgba(255,255,255,.06)', background: 'rgba(0,0,0,.18)' }}>
          <LevelMeter level={audio.level} active={capturing} color={capture.color === '#8f8a83' ? '#7fe0a8' : capture.color} />
          <div className="flex items-baseline justify-between gap-3" data-testid="capture-state">
            <span style={{ font: fnt(500, 12), color: capture.color }}>{capture.value}</span>
            <span className="truncate text-right" style={{ font: fnt(400, 11), color: '#6e6961' }}>
              {capture.detail}
            </span>
          </div>
          {audio.status.fallbackReason && <span style={{ font: fnt(400, 10.5, 'sans', 1.45), color: '#f0cfa6' }}>Raison du repli : {audio.status.fallbackReason}</span>}
        </div>
      )}
      <div className="flex items-center gap-3.5 pt-1.5">
        <div
          className="pill pill-ember flex-none px-4 py-[9px]"
          role="button"
          onClick={() => (native ? runCaptureTest(s.capture) : s.runTest())}
          style={{ font: fnt(500, 12.5) }}
        >
          {testing ? 'Écoute…' : native ? 'Tester la capture 10 s' : 'Test audio 10 s'}
        </div>
        <div className="flex flex-1 flex-col gap-1.5">
          <div className="h-0.5 overflow-hidden rounded-[1px]" style={{ background: 'rgba(255,255,255,.07)' }}>
            <div className="h-0.5" data-testid="test-progress" style={{ width: `${progress}%`, background: '#ffa24a', boxShadow: '0 0 10px #ffa24a', transition: 'width .25s linear' }} />
          </div>
          <span style={{ font: fnt(400, 11), color: native && audio.test.error ? '#ff8a7a' : '#6e6961' }}>
            {native
              ? testMessage(audio.test)
              : s.testDone
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
  const native = useSpeech((x) => x.native);
  const stats = useSpeech((x) => x.stats);
  const activity = useSpeech((x) => x.activity);
  return (
    <div className="glass flex flex-col gap-3.5 p-5">
      <div className="flex items-center justify-between">
        <span className="lbl">DÉTECTION DE PAROLE · VAD{native ? ' SILERO' : ''}</span>
        {native && (
          <span data-testid="vad-activity" style={{ font: fnt(500, 9.5, 'mono'), letterSpacing: '.12em', color: activity.speaking ? '#ffa24a' : '#5a5650' }}>
            {activity.speaking ? '● PAROLE' : 'À L’ÉCOUTE'}
          </span>
        )}
      </div>
      <Slider label="Seuil de détection" unit="%" min={20} max={90} value={vad} onChange={(v) => set({ vad: v })} hint="Plus haut = ignore davantage les tirs et bruitages" />
      {native ? (
        <div className="flex flex-col gap-1.5" style={{ font: fnt(400, 11, 'sans', 1.5), color: '#6e6961' }}>
          <span>Pré-roll 256 ms · post-roll 416 ms · 8 s max · rejet des tirs et explosions · segments de plus de 6 s abandonnés.</span>
          <span data-testid="speech-stats" style={{ color: '#8f8a83' }}>
            {statsLine(stats)}
          </span>
          <span>La VAD ne sépare pas parfaitement les voix des bruitages : un filtre anti-hallucinations vérifie aussi chaque transcription.</span>
        </div>
      ) : (
        <div className="flex items-center justify-between gap-3">
          <div className="flex flex-col gap-[3px]">
            <span className="row-title">
              Suppression de bruit <span style={{ font: fnt(500, 9, 'mono'), letterSpacing: '.12em', color: '#ffa24a', marginLeft: 6 }}>EXPÉRIMENTAL</span>
            </span>
            <span className="row-sub">Pré-roll 250 ms · post-roll 400 ms</span>
          </div>
          <Toggle on={noise} onClick={() => toggle('noise')} label="Suppression de bruit" />
        </div>
      )}
    </div>
  );
}

/** Desktop app: the real model files found on disk and the engine state. */
function NativeModelCard() {
  const s = useApp();
  const sp = useSpeech();
  const engine = engineSummary(sp.status);
  const stats = sp.stats;
  return (
    <div className="glass flex flex-col gap-2 p-5">
      <div className="flex items-center justify-between pb-1">
        <span className="lbl">MODÈLE LOCAL · WHISPER</span>
        <span style={{ font: fnt(500, 9.5, 'mono'), letterSpacing: '.12em', color: '#6e6961' }}>{sp.status?.backend ?? 'CPU'} · SANS GPU</span>
      </div>
      <div className="flex items-baseline justify-between gap-3 pb-1" data-testid="engine-state">
        <span style={{ font: fnt(500, 12.5), color: engine.color }}>{engine.value}</span>
        <span className="truncate text-right" style={{ font: fnt(400, 11), color: '#6e6961' }}>
          {engine.detail}
        </span>
      </div>
      {sp.models.map((m) => {
        const on = s.model === m.id;
        return (
          <div
            key={m.id}
            role="radio"
            aria-checked={on}
            aria-disabled={!m.installed}
            onClick={() => s.set({ model: m.id as ModelId })}
            className="grid items-center gap-3 rounded-[10px] px-3 py-2.5"
            style={{
              gridTemplateColumns: '15px minmax(0,1fr) auto',
              border: `1px solid ${on ? 'rgba(255,162,74,.35)' : 'rgba(255,255,255,.05)'}`,
              background: on ? 'rgba(255,162,74,.06)' : 'transparent',
              cursor: 'pointer',
              opacity: m.installed || on ? 1 : 0.55,
              transition: 'all .3s',
            }}
          >
            <Radio on={on} size={13} dot={5} />
            <div className="flex min-w-0 flex-col gap-0.5">
              <span style={{ font: fnt(500, 12.5, 'mono'), color: '#ece6dc' }}>whisper-{m.id}</span>
              <span style={{ font: fnt(400, 11), color: '#6e6961' }}>{m.note}</span>
            </div>
            <div className="flex flex-col items-end gap-0.5">
              <span style={{ font: fnt(400, 11, 'mono'), color: '#8f8a83' }}>{m.sizeMb >= 1000 ? `${(m.sizeMb / 1000).toFixed(1).replace('.', ',')} Go` : `${m.sizeMb} Mo`}</span>
              <span style={{ font: fnt(500, 10.5), color: on ? '#ffa24a' : m.installed ? '#7fe0a8' : '#6e6961' }}>{on ? (m.installed ? 'Actif' : 'Sélectionné · téléchargement') : m.installed ? 'Installé' : 'Télécharger'}</span>
            </div>
          </div>
        );
      })}
      {stats && stats.transcribed > 0 && (
        <div style={{ font: fnt(400, 11, 'sans', 1.5), color: '#8f8a83' }} data-testid="asr-metrics">
          Latence p50 {stats.latencyP50Ms != null ? formatMs(stats.latencyP50Ms) : '—'} · p95 {stats.latencyP95Ms != null ? formatMs(stats.latencyP95Ms) : '—'}
          {stats.lastRtf != null && ` · RTF ${stats.lastRtf.toFixed(2).replace('.', ',')}`}
          {stats.cpuPercent != null && ` · CPU ${Math.round(stats.cpuPercent)} % de la machine`}
        </div>
      )}
      <div className="flex items-center gap-3 pt-1">
        <div className="pill pill-ghost flex-none px-3.5 py-[7px]" role="button" onClick={() => rescanModels()} style={{ font: fnt(500, 11.5) }}>
          Rechercher les modèles
        </div>
        <span className="min-w-0 truncate" title={sp.status?.modelsDir} style={{ font: fnt(400, 10.5, 'mono'), color: '#5a5650' }}>
          {sp.status?.modelsDir ?? '%LOCALAPPDATA%\\Crosstalk\\models'}
        </span>
      </div>
      <div style={{ font: fnt(400, 10.5, 'sans', 1.5), color: '#5a5650' }}>
        Les modèles se téléchargent automatiquement (Hugging Face, vérification SHA-1) : choisissez-en un autre pour le récupérer.
      </div>
    </div>
  );
}

function ModelCard() {
  const s = useApp();
  const native = useSpeech((x) => x.native);
  if (native) return <NativeModelCard />;
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

const TEST_SAMPLES: Record<'ru' | 'en', string> = {
  ru: 'Двое идут на B, у них AWP.',
  en: 'Two players on B, they have an AWP.',
};

/** Local translation: Opus-MT on this PC, no API key, nothing sent online. */
export function TranslationCard() {
  const on = useApp((s) => s.tg.translate);
  const toggle = useApp((s) => s.toggle);
  const tr = useTranslation();
  const [lang, setLang] = useState<'ru' | 'en'>('ru');
  const [text, setText] = useState(TEST_SAMPLES.ru);
  const [busy, setBusy] = useState(false);
  const [result, setResult] = useState<Translation | null>(null);
  const p50 = percentile(tr.times, 0.5);
  const p95 = percentile(tr.times, 0.95);
  const test = async () => {
    setBusy(true);
    setResult(null);
    try {
      setResult(await nativeTranslation.test(lang, text));
    } finally {
      setBusy(false);
    }
  };
  return (
    <div className="glass flex flex-col gap-3 p-5" data-testid="translation-card">
      <div className="flex items-center justify-between">
        <span className="lbl">TRADUCTION LOCALE · OPUS-MT</span>
        <span style={{ font: fnt(500, 9.5, 'mono'), letterSpacing: '.12em', color: '#6e6961' }}>CPU · SANS CLÉ API</span>
      </div>
      <div style={{ font: fnt(400, 11, 'sans', 1.5), color: '#8f8a83' }}>
        Les voix russes et anglaises sont traduites en français sur ce PC. Aucune clé API, aucun abonnement, rien n’est envoyé en ligne.
      </div>
      <ToggleRow title="Traduire les voix en français" sub="Sinon, la transcription reste en langue originale" on={on} onClick={() => toggle('translate')} />
      {tr.native ? (
        <>
          {(tr.status?.pairs ?? []).map((p) => {
            const sum = pairSummary(p);
            return (
              <div key={p.source} className="flex items-baseline justify-between gap-3" data-testid={`pair-${p.source}`}>
                <span style={{ font: fnt(500, 12.5), color: '#ece6dc' }}>{sum.label}</span>
                <span className="truncate text-right" title={p.repo} style={{ font: fnt(400, 11), color: sum.color }}>
                  {sum.value}
                </span>
              </div>
            );
          })}
          {p50 != null && (
            <div style={{ font: fnt(400, 11, 'sans', 1.5), color: '#8f8a83' }} data-testid="translation-metrics">
              Traduction p50 {formatMs(p50)} · p95 {formatMs(p95 ?? p50)} · {tr.times.length} phrases
            </div>
          )}
          <div className="flex flex-col gap-2 pt-1">
            <div className="flex items-center gap-2">
              <Seg
                options={[
                  ['ru', 'RU'],
                  ['en', 'EN'],
                ]}
                value={lang}
                onChange={(v) => {
                  setLang(v);
                  setText(TEST_SAMPLES[v]);
                  setResult(null);
                }}
                item={{ padding: '4px 10px', font: fnt(500, 11) }}
              />
              <input className="text-input min-w-0 flex-1" aria-label="Phrase à traduire" value={text} maxLength={500} onChange={(e) => setText(e.target.value)} />
              <div className="pill pill-ember flex-none px-3.5 py-[7px]" role="button" aria-disabled={busy || !text.trim()} onClick={() => !busy && text.trim() && test()} style={{ font: fnt(500, 11.5) }}>
                {busy ? 'Traduction…' : 'Traduire'}
              </div>
            </div>
            {result && (
              <div role="status" data-testid="translation-result" style={{ font: fnt(400, 12, 'sans', 1.5), color: result.translated ? '#ece6dc' : '#ff8a7a' }}>
                {result.translated ? `« ${result.translated} » · ${formatMs(result.ms)}${result.cached ? ' · cache' : ''}` : `Non traduit : ${result.error}`}
              </div>
            )}
          </div>
          <div className="flex items-center gap-3">
            <div className="pill pill-ghost flex-none px-3.5 py-[7px]" role="button" onClick={() => rescanTranslation()} style={{ font: fnt(500, 11.5) }}>
              Rechercher les modèles
            </div>
            <span className="min-w-0 truncate" style={{ font: fnt(400, 10.5, 'mono'), color: '#5a5650' }}>
              opus-mt-ru-fr · opus-mt-en-fr
            </span>
          </div>
          <div style={{ font: fnt(400, 10.5, 'sans', 1.5), color: '#5a5650' }}>
            Modèles téléchargés automatiquement au premier lancement (environ 620 Mo). Helsinki-NLP Opus-MT, licence Apache-2.0.
          </div>
        </>
      ) : (
        <div style={{ font: fnt(400, 11, 'sans', 1.5), color: '#6e6961' }}>Disponible dans l’application Windows : modèles Opus-MT installés sur le PC.</div>
      )}
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
        <TranslationCard />
      </div>
    </div>
  );
}
