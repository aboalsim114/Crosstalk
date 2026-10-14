import { motion, type Variants } from 'motion/react';
import { useAudio } from '../../features/audio/audio';
import { engineSummary, useSpeech } from '../../features/speech/speech';
import { useApp } from '../state/store';
import { translationSummary, useTranslation } from '../../features/translation/translation';
import { EASE, fnt } from '../../shared/ui/atoms';
import { IN_LANGUAGE_FR } from '../../features/languages/langs';

// Each [data-reveal] element of the mockup fades up out of a blur, 160 ms apart, after 500 ms.
const container: Variants = { show: { transition: { delayChildren: 0.5, staggerChildren: 0.16 } } };
const reveal: Variants = {
  hidden: { opacity: 0, y: 18, filter: 'blur(10px)' },
  show: { opacity: 1, y: 0, filter: 'blur(0px)', transition: { duration: 1.3, ease: EASE } },
};

/** The readiness line under the title: real states in the desktop app. */
export function introStatus(
  native: boolean,
  cs2Running: boolean | undefined,
  engine: { value: string },
  translation: { value: string },
): string[] {
  if (!native) return ['CS2 DÉTECTÉ', 'CONSOLE.LOG ACTIF', 'WHISPER SMALL PRÊT'];
  return [cs2Running ? 'CS2 DÉTECTÉ' : 'CS2 NON LANCÉ', engine.value.toUpperCase(), `TRADUCTION ${translation.value.toUpperCase()}`];
}

export function Intro() {
  const target = useApp((s) => s.target);
  const entered = useApp((s) => s.entered);
  const audio = useAudio();
  const engine = engineSummary(useSpeech((x) => x.status));
  const tr = translationSummary(useTranslation((x) => x.status));
  const status = introStatus(audio.native, audio.cs2?.running, engine, tr);
  const { enter, enterStart } = useApp.getState();
  return (
    <motion.div
      variants={container}
      initial="hidden"
      animate="show"
      className="absolute inset-0 z-[3] flex flex-col justify-between px-16 pt-[84px] pb-[92px]"
      style={{ opacity: entered ? 0 : 1, transition: 'opacity .9s ease', pointerEvents: entered ? 'none' : 'auto' }}
      data-testid="intro"
    >
      <div className="flex justify-between" style={{ font: fnt(500, 10, 'mono'), letterSpacing: '.22em', color: '#6e6961' }}>
        <motion.span variants={reveal}>UN OUTIL EXTERNE · AUCUNE INJECTION · AUCUNE LECTURE MÉMOIRE</motion.span>
        <motion.span variants={reveal}>V0.2 — MMXXVI</motion.span>
      </div>
      <div className="flex flex-col items-center gap-[18px] text-center">
        <motion.div variants={reveal} style={{ font: fnt(500, 10.5, 'mono'), letterSpacing: '.3em', color: '#ffa24a' }}>
          CROSSTALK
        </motion.div>
        <motion.div variants={reveal} style={{ font: fnt(400, 76, 'serif', 0.98), color: '#f3ede3', letterSpacing: '-.02em', textShadow: '0 4px 40px rgba(0,0,0,.9)' }}>
          Chaque call,
          <br />
          <span style={{ fontStyle: 'italic', color: '#ffc68a' }}>{IN_LANGUAGE_FR[target]}.</span>
        </motion.div>
        <motion.div variants={reveal} className="max-w-[520px]" style={{ font: fnt(300, 15, 'sans', 1.6), color: '#a49d92' }}>
          Les voix et le chat de vos coéquipiers, traduits en temps réel dans votre langue, dans un overlay discret.
        </motion.div>
        <motion.div variants={reveal} className="flex items-center gap-3 pt-2.5">
          <div
            role="button"
            onClick={enter}
            className="cursor-pointer rounded-full px-[30px] py-[13px] transition-colors hover:!bg-[#ffb46a]"
            style={{ background: '#ffa24a', color: '#120c06', font: fnt(600, 13.5), boxShadow: '0 0 40px rgba(255,162,74,.45)' }}
          >
            Entrer
          </div>
          <div role="button" onClick={enterStart} className="pill pill-ghost px-6 py-[13px]" style={{ borderColor: 'rgba(255,255,255,.14)', color: '#ece6dc', font: fnt(500, 13.5) }}>
            Entrer et démarrer la session
          </div>
        </motion.div>
      </div>
      <motion.div variants={reveal} className="flex justify-center gap-7" style={{ font: fnt(500, 10, 'mono'), letterSpacing: '.18em', color: '#5a5650' }}>
        {status.map((t, i) => (
          <span key={t} className="flex gap-7">
            {i > 0 && <span>·</span>}
            <span>{t}</span>
          </span>
        ))}
      </motion.div>
    </motion.div>
  );
}

/** Black cinema bars that retract when the user enters. */
export function Letterbox() {
  const entered = useApp((s) => s.entered);
  const h = entered ? 0 : 58;
  const style = { height: h, transition: 'height 1.3s cubic-bezier(.7,0,.2,1)' };
  return (
    <>
      <div className="pointer-events-none absolute top-0 right-0 left-0 z-[4] bg-black" style={style} />
      <div className="pointer-events-none absolute right-0 bottom-0 left-0 z-[4] bg-black" style={style} />
    </>
  );
}

export function Toast() {
  const toast = useApp((s) => s.toast);
  if (!toast) return null;
  return (
    <div
      role="status"
      className="absolute bottom-7 left-1/2 z-[8] -translate-x-1/2 rounded-full px-[22px] py-3"
      style={{ background: 'rgba(236,230,220,.95)', color: '#0a0908', font: fnt(500, 12.5), boxShadow: '0 10px 40px rgba(0,0,0,.6)' }}
    >
      {toast}
    </div>
  );
}
