import { AnimatePresence, motion } from 'motion/react';
import { config } from '../config';
import { EASE, Seg, fmtSeconds, fnt } from '../components/atoms';
import { SIGNAL_THRESHOLD_DB, captureSummary, cs2Summary, isCapturing, useAudio } from '../state/audio';
import { engineSummary, useSpeech } from '../state/speech';
import { translationSummary, useTranslation } from '../state/translation';
import { isLive, useApp, type FeedMode } from '../state/store';
import { feedRow, isDemoLine, subtitle, voiceLatency } from './derive';

const MODES: [FeedMode, string][] = [
  ['text', 'Texte'],
  ['voice', 'Voix'],
  ['mixed', 'Mixte'],
];

function Subtitle() {
  const f0 = useApp((s) => s.feed[0]);
  const live = useApp(isLive);
  const sub = subtitle(f0, live);
  const demo = isDemoLine(useAudio((a) => a.native), f0);
  return (
    <motion.div
      key={f0?.id ?? 'idle'}
      initial={f0 ? { opacity: 0, y: 16, filter: 'blur(12px)' } : false}
      animate={{ opacity: 1, y: 0, filter: 'blur(0px)' }}
      transition={{ duration: 0.9, ease: EASE }}
      className="px-5 pb-[26px] text-center"
      data-testid="subtitle"
    >
      <motion.div
        key={f0?.status ?? 'none'}
        initial={f0 ? { opacity: 0.6, filter: 'blur(3px)' } : false}
        animate={{ opacity: 1, filter: 'blur(0px)' }}
        transition={{ duration: 0.5, ease: EASE }}
        className="flex flex-col items-center gap-3"
      >
        <div className="flex items-center gap-2.5" style={{ font: fnt(500, 10.5, 'mono'), letterSpacing: '.16em', color: sub.color }}>
          {demo && <span style={{ color: '#ffa24a' }}>DÉMO ·</span>}
          <span>{sub.tag}</span>
          <span style={{ color: '#5a5650' }}>{sub.sender}</span>
        </div>
        <div className="max-w-[560px]" style={{ font: fnt(400, 13.5, 'mono', 1.5), color: '#8f8a83' }}>
          {sub.orig}
        </div>
        <div
          className="max-w-[600px]"
          style={{
            font: fnt(400, 44, 'serif', 1.12),
            fontStyle: sub.italic ? 'italic' : 'normal',
            color: sub.trColor,
            textWrap: 'balance',
            textShadow: '0 2px 30px rgba(0,0,0,.9)',
            transition: 'color .6s',
          }}
        >
          {sub.text}
        </div>
        <div style={{ font: fnt(400, 11.5), color: sub.noteColor }}>{sub.note}</div>
      </motion.div>
    </motion.div>
  );
}

function Pipeline() {
  const s = useApp();
  const live = isLive(s);
  const audio = useAudio();
  const sp = useSpeech();
  const tr = useTranslation((x) => x.status);
  const translating = s.feed.some((f) => f.tid != null && f.status === 'transcrit' && !f.note && !f.live);
  const fb = config.win10Fallback;
  const captureSub = audio.native ? captureSummary(audio.status).short : fb ? 'loopback' : 'cs2.exe';
  const stages: [string, string][] = [
    ['Capture', captureSub],
    ['VAD', audio.native ? `Silero · seuil ${s.vad} %` : `seuil ${s.vad} %`],
    ['Transcription', audio.native ? `whisper ${s.model} · ${sp.status?.state === 'ready' ? 'prêt' : engineSummary(sp.status).value.toLowerCase()}` : `whisper ${s.model}`],
    ['Traduction', audio.native ? translationSummary(tr).value : 'opus-mt → fr'],
    ['Overlay', s.fmt],
  ];
  return (
    <div className="flex items-center px-2 pb-[22px]">
      {stages.map(([name, sub], i) => {
        const running = live && !s.paused;
        // Desktop app: real states (capture, voice detected, Whisper running, a line awaiting its translation).
        const nativeActive = [isCapturing(audio.status), sp.activity.speaking, sp.activity.transcribing, translating, false][i];
        const active = audio.native ? running && nativeActive : running && (i === 0 || s.stage === i);
        const done = !audio.native && running && i > 0 && s.stage > i;
        const col = i === 0 ? '#7fe0a8' : '#ffa24a';
        return (
          <div key={name} className="flex flex-1 flex-col gap-2" data-active={active || undefined}>
            <div
              className="mr-1.5 h-0.5 rounded-[1px]"
              style={{ background: active ? col : done ? 'rgba(255,162,74,.35)' : 'rgba(255,255,255,.08)', boxShadow: active ? `0 0 12px ${col}` : 'none', transition: 'all .35s' }}
            />
            <div className="flex flex-col gap-0.5">
              <span style={{ font: fnt(500, 11.5), color: active ? '#ece6dc' : '#6e6961', transition: 'color .35s' }}>{name}</span>
              <span style={{ font: fnt(400, 10, 'mono'), color: '#5a5650' }}>{sub}</span>
            </div>
          </div>
        );
      })}
    </div>
  );
}

function StatusCards() {
  const s = useApp();
  const live = isLive(s);
  const fb = config.win10Fallback;
  const down = config.translatorDown;
  const lat = voiceLatency(s.feed);
  const audio = useAudio();
  const cs2 = cs2Summary(audio.cs2);
  const capture = captureSummary(audio.status);
  const engine = engineSummary(useSpeech((x) => x.status));
  const trSum = translationSummary(useTranslation((x) => x.status));
  const cards = [
    audio.native
      ? { label: 'COUNTER-STRIKE 2', value: cs2.value, sub: cs2.detail, dot: cs2.color }
      : { label: 'COUNTER-STRIKE 2', value: live ? 'En cours' : 'Détecté', sub: 'console.log · -condebug actif', dot: '#7fe0a8' },
    audio.native
      ? { label: 'CAPTURE', value: capture.value, sub: capture.detail, dot: capture.color }
      : { label: 'CAPTURE', value: fb ? 'Loopback système' : 'Process loopback', sub: fb ? 'Repli — tout le périphérique' : 'Son de cs2.exe uniquement', dot: fb ? '#ffa24a' : '#7fe0a8' },
    audio.native
      ? { label: 'TRANSCRIPTION', value: engine.value, sub: `${engine.detail} · traduction ${trSum.value}`, dot: engine.color }
      : { label: 'MOTEURS', value: `Whisper ${s.model}`, sub: `${s.accel.toUpperCase()} · Opus-MT${down ? ' indisponible' : ''}`, dot: down ? '#ff8a7a' : '#7fe0a8' },
    {
      label: 'LATENCE VOIX',
      value: lat ? `p50 ${fmtSeconds(lat.p50)}` : '—',
      sub: lat ? `p95 ${fmtSeconds(lat.p95)} · ${lat.n} phrases` : 'Objectif p50 ≤ 2 s · p95 ≤ 4 s',
      dot: lat ? '#7fe0a8' : '#3d3a36',
    },
  ];
  return (
    <div className="grid grid-cols-4 pt-4" style={{ borderTop: '1px solid rgba(255,255,255,.07)' }}>
      {cards.map((c) => (
        <div key={c.label} className="flex flex-col gap-1.5 pr-3.5">
          <div className="flex items-center gap-[7px]" style={{ font: fnt(500, 9.5, 'mono'), letterSpacing: '.16em', color: '#6e6961' }}>
            <div className="h-[5px] w-[5px] rounded-[3px]" style={{ background: c.dot }} />
            {c.label}
          </div>
          <div style={{ font: fnt(400, 21, 'serif', 1.1), color: '#ece6dc' }}>{c.value}</div>
          <div style={{ font: fnt(400, 11, 'sans', 1.35), color: '#6e6961' }}>{c.sub}</div>
        </div>
      ))}
    </div>
  );
}

function ScriptPanel() {
  const feed = useApp((s) => s.feed);
  const mode = useApp((s) => s.mode);
  const live = useApp(isLive);
  const set = useApp((s) => s.set);
  const native = useAudio((a) => a.native);
  return (
    <div className="glass flex min-h-0 flex-col overflow-hidden">
      <div className="flex items-center justify-between px-[18px] pt-4 pb-3.5" style={{ borderBottom: '1px solid rgba(255,255,255,.05)' }}>
        <div className="flex items-baseline gap-2.5">
          <span style={{ font: fnt(400, 22, 'serif'), color: '#ece6dc' }}>Script</span>
          <span style={{ font: fnt(400, 10.5, 'mono'), color: '#5a5650' }}>{feed.length ? `${feed.length} répliques` : ''}</span>
          {native && (
            <span
              title="Les voix sont réellement transcrites puis traduites sur ce PC. Le chat écrit est encore simulé : ces lignes sont des exemples."
              style={{ font: fnt(600, 9, 'mono'), letterSpacing: '.14em', color: '#ffa24a', border: '1px solid rgba(255,162,74,.35)', borderRadius: 999, padding: '2px 7px' }}
            >
              CHAT SIMULÉ
            </span>
          )}
        </div>
        <Seg options={MODES} value={mode} onChange={(v) => set({ mode: v })} item={{ padding: '4px 10px', font: fnt(500, 11) }} />
      </div>
      <div className="flex-1 overflow-auto" data-testid="feed">
        {!feed.length && (
          <div className="flex h-full flex-col items-center justify-center gap-2.5 px-[30px] py-10 text-center">
            <div style={{ font: fnt(400, 22, 'serif'), fontStyle: 'italic', color: '#bdb6ab' }}>{live ? 'En écoute…' : 'Rien à l’écran.'}</div>
            <div style={{ font: fnt(400, 12, 'sans', 1.6), color: '#6e6961' }}>
              {live ? (native ? 'Chaque phrase transcrite s’inscrit ici, dans sa langue d’origine.' : 'Chaque réplique traduite s’inscrit ici.') : 'Ctrl+Alt+F9 met en pause, Ctrl+Alt+F10 coupe toutes les captures.'}
            </div>
          </div>
        )}
        <AnimatePresence initial={false}>
          {feed.map((f) => {
            const e = feedRow(f);
            return (
              <motion.div
                key={e.id}
                initial={{ opacity: 0, y: -10, filter: 'blur(6px)' }}
                animate={{ opacity: 1, y: 0, filter: 'blur(0px)' }}
                transition={{ duration: 0.7, ease: EASE }}
                className="flex flex-col gap-1.5 px-[18px] py-3.5"
                style={{ borderBottom: '1px solid rgba(255,255,255,.04)' }}
              >
                <div className="flex items-center justify-between" style={{ font: fnt(500, 9.5, 'mono'), letterSpacing: '.12em' }}>
                  <span style={{ color: e.color }}>
                    {e.tag} <span style={{ color: '#5a5650' }}>· {e.langs}</span>
                  </span>
                  <span style={{ color: e.statusColor }}>
                    {e.status} <span style={{ color: '#5a5650' }}>{e.lat}</span>
                  </span>
                </div>
                <div style={{ font: fnt(400, 14.5, 'sans', 1.4), color: e.trColor, fontStyle: e.italic ? 'italic' : 'normal', textWrap: 'pretty' }}>
                  {e.lead}
                  {e.shown}
                </div>
                <div style={{ font: fnt(400, 11.5, 'mono', 1.45), color: '#6e6961' }}>{e.orig}</div>
                <div style={{ font: fnt(400, 10.5), color: e.noteColor }}>{e.note}</div>
              </motion.div>
            );
          })}
        </AnimatePresence>
      </div>
      <div className="flex items-center gap-2.5 px-[18px] py-3" style={{ borderTop: '1px solid rgba(255,255,255,.05)', font: fnt(400, 11, 'sans', 1.45), color: '#8f8a83' }}>
        <span style={{ font: fnt(600, 9.5, 'mono'), letterSpacing: '.14em', color: '#7fe0a8' }}>LOCAL</span>
        <span>Aucune voix transmise. Aucun audio conservé. Micro coupé.</span>
      </div>
    </div>
  );
}

export function Dashboard() {
  const live = useApp(isLive);
  const session = useApp((s) => s.session);
  const speaking = useApp((s) => s.speaking);
  const audio = useAudio();
  const fb = config.win10Fallback;
  const capturing = isCapturing(audio.status);
  const scene = audio.native
    ? capturing
      ? `SCÈNE EN DIRECT · ${captureSummary(audio.status).short.toUpperCase()}`
      : session === 'PAUSED'
        ? 'SCÈNE EN PAUSE'
        : 'AUCUNE SCÈNE'
    : live
      ? fb
        ? 'SCÈNE EN DIRECT · CAPTURE DE REPLI'
        : 'SCÈNE EN DIRECT · CS2.EXE'
      : session === 'PAUSED'
        ? 'SCÈNE EN PAUSE'
        : 'AUCUNE SCÈNE';
  // In the desktop app: real voice detection (Silero), else real signal presence.
  const activity = useSpeech((x) => x.activity);
  const signal = capturing && audio.level !== null && audio.level.rmsDb > SIGNAL_THRESHOLD_DB;
  const indicator = audio.native
    ? capturing && activity.speaking
      ? { text: '● PAROLE DÉTECTÉE', color: '#ffa24a' }
      : { text: capturing ? (signal ? '● SIGNAL AUDIO' : 'silence') : '', color: signal ? '#7fe0a8' : '#4a4640' }
    : { text: !live ? '' : speaking ? '● PAROLE DÉTECTÉE' : 'bruit ignoré', color: speaking ? '#ffa24a' : '#4a4640' };
  return (
    <div className="grid h-full gap-5" style={{ gridTemplateColumns: 'minmax(0,1fr) 372px' }}>
      <div className="flex min-w-0 flex-col">
        <div className="flex justify-between" style={{ font: fnt(500, 10, 'mono'), letterSpacing: '.16em', color: '#5a5650' }}>
          <span>{scene}</span>
          <span style={{ color: indicator.color }} data-testid="signal-indicator">
            {indicator.text}
          </span>
        </div>
        <div className="flex-1" />
        <Subtitle />
        <Pipeline />
        <StatusCards />
      </div>
      <ScriptPanel />
    </div>
  );
}
