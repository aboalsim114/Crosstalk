import { AnimatePresence, motion } from 'motion/react';
import { config } from '../../app/config';
import { t, useT, type MessageKey } from '../../shared/i18n';
import { EASE, Seg, fmtSeconds, fnt } from '../../shared/ui/atoms';
import { SIGNAL_THRESHOLD_DB, captureSummary, cs2Summary, isCapturing, useAudio } from '../audio/audio';
import { engineSummary, useSpeech } from '../speech/speech';
import { translationSummary, useTranslation } from '../translation/translation';
import { cs2SetupSummary, useChat } from '../chat/chat';
import { isLive, useApp, type FeedMode } from '../../app/state/store';
import { feedRow, isDemoLine, latencyAdvice, subtitle, voiceLatency } from './derive';

const MODES: [FeedMode, MessageKey][] = [
  ['text', 'dash.mode.text'],
  ['voice', 'dash.mode.voice'],
  ['mixed', 'dash.mode.mixed'],
];

function Subtitle() {
  useT(); // re-render when the language changes
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
          {demo && <span style={{ color: '#ffa24a' }}>{t('dash.demo')}</span>}
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
  const t = useT();
  const s = useApp();
  const live = isLive(s);
  const audio = useAudio();
  const sp = useSpeech();
  const tr = useTranslation((x) => x.status);
  const translating = s.feed.some((f) => f.tid != null && f.status === 'transcrit' && !f.note && !f.live);
  const fb = config.win10Fallback;
  const captureSub = audio.native ? captureSummary(audio.status).short : fb ? 'loopback' : 'cs2.exe';
  const threshold = t('dash.vad.threshold', { n: s.vad });
  const stages: [string, string][] = [
    [t('dash.stage.capture'), captureSub],
    [t('dash.stage.vad'), audio.native ? `Silero · ${threshold}` : threshold],
    [t('dash.stage.asr'), audio.native ? `whisper ${s.model} · ${sp.status?.state === 'ready' ? t('dash.ready') : engineSummary(sp.status).value.toLowerCase()}` : `whisper ${s.model}`],
    [t('dash.stage.translation'), audio.native ? translationSummary(tr).value : 'LLM → fr'],
    [t('dash.stage.overlay'), t(`ov.fmt.${s.fmt}`)],
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
  const t = useT();
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
  const chatSum = cs2SetupSummary(useChat((x) => x.status));
  const cards = [
    audio.native
      ? { label: t('dash.card.cs2'), value: cs2.value, sub: t('dash.cs2.sub', { detail: cs2.detail, chat: chatSum.value }), dot: cs2.color }
      : { label: t('dash.card.cs2'), value: live ? t('dash.demo.running') : t('dash.demo.detected'), sub: t('dash.demo.condebug'), dot: '#7fe0a8' },
    audio.native
      ? { label: t('dash.card.capture'), value: capture.value, sub: capture.detail, dot: capture.color }
      : {
          label: t('dash.card.capture'),
          value: fb ? t('dash.demo.fallback') : t('dash.demo.process'),
          sub: fb ? t('dash.demo.fallbackSub') : t('dash.demo.processSub'),
          dot: fb ? '#ffa24a' : '#7fe0a8',
        },
    audio.native
      ? { label: t('dash.card.asr'), value: engine.value, sub: t('dash.asr.sub', { detail: engine.detail, tr: trSum.value }), dot: engine.color }
      : { label: t('dash.card.engines'), value: `Whisper ${s.model}`, sub: `${s.accel.toUpperCase()} · LLM${down ? ` ${t('dash.demo.unavailable')}` : ''}`, dot: down ? '#ff8a7a' : '#7fe0a8' },
    {
      label: t('dash.card.latency'),
      value: lat ? `p50 ${fmtSeconds(lat.p50)}` : '—',
      sub: lat ? (latencyAdvice(lat.p50, s.model) ?? t('dash.latency.sub', { p95: fmtSeconds(lat.p95), count: lat.n })) : t('dash.latency.goal'),
      dot: !lat ? '#3d3a36' : latencyAdvice(lat.p50, s.model) ? '#ffa24a' : '#7fe0a8',
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
  const t = useT();
  const feed = useApp((s) => s.feed);
  const mode = useApp((s) => s.mode);
  const live = useApp(isLive);
  const set = useApp((s) => s.set);
  const native = useAudio((a) => a.native);
  return (
    <div className="glass flex min-h-0 flex-col overflow-hidden">
      <div className="flex items-center justify-between px-[18px] pt-4 pb-3.5" style={{ borderBottom: '1px solid rgba(255,255,255,.05)' }}>
        <div className="flex items-baseline gap-2.5">
          <span style={{ font: fnt(400, 22, 'serif'), color: '#ece6dc' }}>{t('dash.script')}</span>
          <span style={{ font: fnt(400, 10.5, 'mono'), color: '#5a5650' }}>{feed.length ? t('dash.lines', { count: feed.length }) : ''}</span>
        </div>
        <Seg options={MODES.map(([id, k]) => [id, t(k)] as [FeedMode, string])} value={mode} onChange={(v) => set({ mode: v })} item={{ padding: '4px 10px', font: fnt(500, 11) }} />
      </div>
      <div className="flex-1 overflow-auto" data-testid="feed">
        {!feed.length && (
          <div className="flex h-full flex-col items-center justify-center gap-2.5 px-[30px] py-10 text-center">
            <div style={{ font: fnt(400, 22, 'serif'), fontStyle: 'italic', color: '#bdb6ab' }}>{live ? t('dash.listening') : t('dash.nothing')}</div>
            <div style={{ font: fnt(400, 12, 'sans', 1.6), color: '#6e6961' }}>
              {live ? (native ? t('dash.hint.native') : t('dash.hint.demo')) : t('dash.hint.idle')}
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
        <span>{t('dash.local')}</span>
      </div>
    </div>
  );
}

export function Dashboard() {
  const t = useT();
  const live = useApp(isLive);
  const session = useApp((s) => s.session);
  const speaking = useApp((s) => s.speaking);
  const audio = useAudio();
  const fb = config.win10Fallback;
  const capturing = isCapturing(audio.status);
  const scene = audio.native
    ? capturing
      ? t('dash.scene.live', { src: captureSummary(audio.status).short.toUpperCase() })
      : session === 'PAUSED'
        ? t('dash.scene.paused')
        : t('dash.scene.none')
    : live
      ? t('dash.scene.live', { src: fb ? t('dash.scene.fallback') : 'CS2.EXE' })
      : session === 'PAUSED'
        ? t('dash.scene.paused')
        : t('dash.scene.none');
  // In the desktop app: real voice detection (Silero), else real signal presence.
  const activity = useSpeech((x) => x.activity);
  const signal = capturing && audio.level !== null && audio.level.rmsDb > SIGNAL_THRESHOLD_DB;
  const indicator = audio.native
    ? capturing && activity.speaking
      ? { text: t('dash.speech'), color: '#ffa24a' }
      : { text: capturing ? (signal ? t('dash.signal') : t('dash.silence')) : '', color: signal ? '#7fe0a8' : '#4a4640' }
    : { text: !live ? '' : speaking ? t('dash.speech') : t('dash.noise'), color: speaking ? '#ffa24a' : '#4a4640' };
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
