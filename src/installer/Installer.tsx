// "Crosstalk Installer" (Claude Design), wired to the real installation.
// Markup and styles follow the design file; the simulated values (folder
// list, required space, progress, launch) come from the Rust installer.

import { useCallback, useEffect, useRef, useState, type CSSProperties, type MouseEvent, type ReactNode } from 'react';
import { native, setup, size, type SetupInfo, type SetupProgress } from './api';
import { css } from './css';
import { createStage, type Stage, type StageState } from './stage';
import { PHASES, phaseOf, phaseSub } from './model';

const EASE = 'cubic-bezier(.16,1,.3,1)';
const A = '#FF643B';
const STEPS = ['BIENVENUE', 'CONFIGURATION', 'INSTALLATION', 'PRÊT'];

/** A design button: inline style plus its hover / active variants. */
function Btn({ s, hover, active, children, ...rest }: { s: string; hover?: string; active?: string; children?: ReactNode } & Omit<React.ButtonHTMLAttributes<HTMLButtonElement>, 'style'>) {
  const [h, setH] = useState(false);
  const [a, setA] = useState(false);
  const style: CSSProperties = { ...css(s), ...(h && hover ? css(hover) : {}), ...(a && active ? css(active) : {}) };
  return (
    <button
      {...rest}
      style={style}
      onMouseEnter={(e) => {
        setH(true);
        rest.onMouseEnter?.(e);
      }}
      onMouseLeave={(e) => {
        setH(false);
        setA(false);
        rest.onMouseLeave?.(e);
      }}
      onMouseDown={() => setA(true)}
      onMouseUp={() => setA(false)}
    >
      {children}
    </button>
  );
}

const glowMove = (e: MouseEvent<HTMLElement>) => {
  const t = e.currentTarget,
    r = t.getBoundingClientRect();
  t.style.setProperty('--mx', ((e.clientX - r.left) / r.width) * 100 + '%');
  t.style.setProperty('--my', ((e.clientY - r.top) / r.height) * 100 + '%');
};

const sw = (on: boolean) => ({
  track: on ? 'rgba(255,100,59,.22)' : 'rgba(242,241,238,.05)',
  border: on ? 'rgba(255,100,59,.6)' : 'rgba(242,241,238,.14)',
  knob: on ? A : '#807F84',
  knobGlow: on ? '0 0 12px rgba(255,100,59,.7)' : 'none',
});

function Word({ children }: { children: ReactNode }) {
  return (
    <span style={css('display:inline-block;overflow:hidden;padding-bottom:.06em')}>
      <span data-word="true" style={css('display:inline-block')}>
        {children}
      </span>
    </span>
  );
}

function Eyebrow({ n, label, style }: { n: string; label: string; style?: string }) {
  return (
    <div data-reveal="true" style={css((style ?? '') + ";display:flex;align-items:center;gap:12px;font:500 10.5px 'JetBrains Mono';letter-spacing:.22em;color:#FF643B")}>
      <span>{n}</span>
      <div style={css('width:28px;height:1px;background:#FF643B')} />
      <span>{label}</span>
    </div>
  );
}

const PRIMARY = (r: number) =>
  `display:flex;align-items:center;border:none;border-radius:999px;cursor:pointer;background:radial-gradient(circle ${r}px at var(--mx,50%) var(--my,50%),rgba(255,236,226,.5),rgba(255,236,226,0) 70%),#FF643B;color:#08090C;letter-spacing:.16em;box-shadow:0 0 0 1px rgba(255,120,80,.6),0 12px 40px -10px rgba(255,100,59,.6);transition:box-shadow .45s ease,transform .3s cubic-bezier(.34,1.56,.64,1)`;
const PRIMARY_HOVER = 'box-shadow:0 0 0 1px rgba(255,160,130,.9),0 16px 60px -8px rgba(255,100,59,.8)';
const GHOST = "border-radius:999px;border:1px solid rgba(242,241,238,.12);background:transparent;color:#A9A8A4;letter-spacing:.14em;cursor:pointer;transition:background .25s,color .25s";
const GHOST_HOVER = 'background:rgba(242,241,238,.05);color:#F2F1EE';

export function Installer({ startScreen = 0 }: { startScreen?: number }) {
  const [screen, setScreen] = useState(startScreen);
  const [info, setInfo] = useState<SetupInfo | null>(null);
  const [path, setPath] = useState('');
  const [free, setFree] = useState<number | null>(null);
  const [desk, setDesk] = useState(true);
  const [launchAfter, setLaunchAfter] = useState(true);
  const [autostart, setAutostart] = useState(false);
  const [progress, setProgress] = useState<SetupProgress | null>(null);
  const [prog, setProg] = useState(0);
  const [termsOpen, setTermsOpen] = useState(false);
  const [confirmOpen, setConfirmOpen] = useState(false);
  const [ended, setEnded] = useState(false);
  const [endSub, setEndSub] = useState('');
  const [launching, setLaunching] = useState(false);
  const [grain, setGrain] = useState<string | null>(null);
  const [no3d, setNo3d] = useState(false);

  const stageRef = useRef<HTMLDivElement>(null);
  const screenRef = useRef<HTMLDivElement>(null);
  const sweepRef = useRef<HTMLDivElement>(null);
  const grainRef = useRef<HTMLDivElement>(null);
  const barTopRef = useRef<HTMLDivElement>(null);
  const barBotRef = useRef<HTMLDivElement>(null);
  const modalRef = useRef<HTMLDivElement>(null);
  const busy = useRef(false);
  const stage = useRef<Stage | null>(null);
  const stageState = useRef<StageState>({ screen: startScreen, prog: 0, reducedMotion: false, mx: 0, my: 0, introStart: performance.now() });
  const rm = () => matchMedia('(prefers-reduced-motion: reduce)').matches;
  stageState.current.screen = screen;
  stageState.current.prog = prog;

  // ---------- real data ----------
  useEffect(() => {
    setup.info().then((i) => {
      setInfo(i);
      setPath(i.defaultDir);
    });
    const un = setup.onProgress(setProgress);
    return () => {
      un.then((f) => f());
    };
  }, []);
  useEffect(() => {
    if (path) setup.freeSpace(path).then(setFree);
  }, [path]);
  // The shown percentage eases toward the real one, like the design's progress.
  const target = useRef(0);
  target.current = progress?.pct ?? 0;
  useEffect(() => {
    if (screen !== 2) return;
    const iv = setInterval(() => {
      setProg((p) => {
        const t = target.current;
        if (p >= t) return t;
        return Math.min(t, p + Math.max(0.25, (t - p) * 0.12));
      });
    }, 50);
    return () => clearInterval(iv);
  }, [screen]);
  const done = !!progress?.done && prog >= 100;
  useEffect(() => {
    if (!done || screen !== 2) return;
    const t = setTimeout(() => go(3), 1300);
    return () => clearTimeout(t);
  });

  // ---------- design motion ----------
  const enter = useCallback((intro: boolean) => {
    const el = screenRef.current;
    if (!el) return;
    const base = intro ? 1500 : 140;
    if (rm()) {
      el.querySelectorAll('[data-reveal],[data-word],[data-logo]').forEach((n) => n.animate([{ opacity: 0 }, { opacity: 1 }], { duration: 300, fill: 'both' }));
      return;
    }
    el.querySelectorAll('[data-logo]').forEach((n) => {
      n.animate([{ opacity: 0, filter: 'blur(10px)' }, { opacity: 1, filter: 'blur(0)' }], { duration: 1400, delay: intro ? 1100 : 60, easing: EASE, fill: 'both' });
      const t = n.querySelector('[data-logo-text]');
      if (t) t.animate([{ letterSpacing: '.9em', opacity: 0 }, { letterSpacing: '.32em', opacity: 1 }], { duration: 1800, delay: intro ? 1100 : 60, easing: EASE, fill: 'both' });
    });
    el.querySelectorAll('[data-word]').forEach((n, k) => n.animate([{ transform: 'translateY(108%) skewY(4deg)' }, { transform: 'none' }], { duration: 1150, delay: base + 200 + k * 85, easing: EASE, fill: 'both' }));
    let i = 0;
    el.querySelectorAll('[data-reveal]').forEach((n) => {
      const d = n.closest('h1') ? 0 : base + i++ * 75;
      n.animate([{ opacity: 0, transform: 'translateY(16px)', filter: 'blur(8px)' }, { opacity: 1, transform: 'none', filter: 'blur(0)' }], { duration: 900, delay: d, easing: EASE, fill: 'both' });
    });
    if (intro)
      [barTopRef.current, barBotRef.current].forEach((b) =>
        b?.animate([{ transform: 'scaleY(1)' }, { transform: 'scaleY(1)', offset: 0.55 }, { transform: 'scaleY(0)' }], { duration: 2800, easing: 'cubic-bezier(.7,0,.2,1)' }),
      );
  }, []);

  useEffect(() => {
    stageState.current.reducedMotion = rm();
    stageState.current.introStart = performance.now() - (startScreen !== 0 ? 4000 : 0);
    const c = document.createElement('canvas');
    c.width = c.height = 180;
    const g = c.getContext('2d');
    if (g) {
      const d = g.createImageData(180, 180);
      for (let i = 0; i < d.data.length; i += 4) {
        const v = Math.random() * 255;
        d.data[i] = d.data[i + 1] = d.data[i + 2] = v;
        d.data[i + 3] = 255;
      }
      g.putImageData(d, 0, 0);
      setGrain(c.toDataURL());
    }
    requestAnimationFrame(() => {
      const gr = grainRef.current;
      if (gr && !rm())
        gr.animate([{ transform: 'translate(0,0)' }, { transform: 'translate(-4%,3%)' }, { transform: 'translate(3%,-2%)' }, { transform: 'translate(-2%,-3%)' }, { transform: 'translate(0,0)' }], {
          duration: 900,
          iterations: Infinity,
          easing: 'steps(5)',
        });
    });
    enter(startScreen === 0);
    if (stageRef.current) {
      stage.current = createStage(stageRef.current, () => stageState.current);
      if (!stage.current) setNo3d(true);
    }
    return () => stage.current?.dispose();
  }, [enter, startScreen]);

  const first = useRef(true);
  useEffect(() => {
    if (first.current) {
      first.current = false;
      return;
    }
    enter(false);
    stage.current?.kick(screen);
  }, [screen, enter]);

  useEffect(() => {
    if ((termsOpen || confirmOpen) && modalRef.current && !rm())
      modalRef.current.animate([{ opacity: 0, transform: 'translateY(10px) scale(.98)', filter: 'blur(6px)' }, { opacity: 1, transform: 'none', filter: 'blur(0)' }], { duration: 520, easing: EASE });
  }, [termsOpen, confirmOpen]);

  const sweep = () => {
    const s = sweepRef.current;
    if (s && !rm()) s.animate([{ transform: 'translateX(-110%)', opacity: 0 }, { opacity: 1, offset: 0.4 }, { transform: 'translateX(190%)', opacity: 0 }], { duration: 1100, easing: 'cubic-bezier(.6,0,.2,1)' });
  };
  const go = (n: number) => {
    if (busy.current) return;
    const el = screenRef.current;
    sweep();
    if (el && !rm()) {
      busy.current = true;
      el.animate([{ opacity: 1, transform: 'none', filter: 'blur(0)' }, { opacity: 0, transform: 'translateY(-10px) scale(.99)', filter: 'blur(8px)' }], { duration: 420, easing: 'cubic-bezier(.4,0,.2,1)', fill: 'forwards' }).finished.then(() => {
        busy.current = false;
        setScreen(n);
      });
    } else setScreen(n);
  };

  // ---------- actions ----------
  const startInstall = () => {
    setProg(0);
    setProgress(null);
    go(2);
    setup.install({ dir: path, desktop: desk }).catch((e) => setProgress({ phase: 0, pct: 0, detail: '', done: false, error: String(e), cancelled: false }));
  };
  const cancel = () => {
    setup.cancel();
    go(1);
  };
  const exitWith = (msg: string) => {
    setEnded(true);
    setEndSub(msg);
    setTimeout(() => setup.quit(), 1600);
  };
  const launch = () => {
    if (launching) return;
    setLaunching(true);
    setup.setAutostart(path, autostart).finally(() => setup.launch(path).then(() => exitWith('Lancement de Crosstalk…')));
  };
  const finish = () => setup.setAutostart(path, autostart).finally(() => exitWith('Installation terminée. Vous pouvez fermer cette fenêtre.'));
  const closeWin = () => {
    if (ended) return;
    if (screen === 3) finish();
    else setConfirmOpen(true);
  };
  const quit = () => {
    setup.cancel();
    setConfirmOpen(false);
    exitWith(screen === 2 ? 'Installation annulée. Les fichiers copiés ont été retirés.' : 'Installation annulée. Aucun fichier n’a été modifié.');
  };

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        setTermsOpen(false);
        setConfirmOpen(false);
        return;
      }
      if (e.key === 'Enter' && !termsOpen && !confirmOpen && !ended && document.activeElement?.tagName !== 'BUTTON') {
        if (screen === 0) go(1);
        else if (screen === 1) startInstall();
        else if (screen === 3) launch();
      }
    };
    addEventListener('keydown', onKey);
    return () => removeEventListener('keydown', onKey);
  });

  // ---------- view values ----------
  const ph = phaseOf(prog, done);
  const error = progress?.error ?? null;
  const NT = 50,
    lit = (prog / 100) * NT;
  const required = info ? info.appBytes + info.modelBytes : null;
  const enough = free == null || required == null || free >= required;
  const statusLabel = error ? 'ERREUR' : done ? 'TERMINÉ' : 'EN COURS';
  const statusColor = error ? '#ff8a7a' : done ? '#91DCEB' : A;
  const as = sw(autostart);
  const launchLabel = launching ? 'LANCEMENT…' : 'LANCER CROSSTALK';

  return (
    <div
      onMouseMove={(e) => {
        const r = e.currentTarget.getBoundingClientRect();
        stageState.current.mx = ((e.clientX - r.left) / r.width) * 2 - 1;
        stageState.current.my = ((e.clientY - r.top) / r.height) * 2 - 1;
      }}
      style={css('width:100vw;height:100vh;display:flex;align-items:center;justify-content:center;overflow:hidden')}
    >
      <div
        data-screen-label="Crosstalk Installer"
        role="dialog"
        aria-label="Installation de Crosstalk"
        style={css(
          'width:1000px;height:640px;flex:none;position:relative;border-radius:14px;overflow:hidden;background:#08090C;border:1px solid rgba(242,241,238,.07);box-shadow:0 0 0 1px rgba(0,0,0,.7),inset 0 1px 0 rgba(242,241,238,.04)',
        )}
      >
        <div ref={stageRef} style={css('position:absolute;inset:0;z-index:0')} />
        {no3d && <div style={css('position:absolute;inset:0;z-index:0;background:radial-gradient(38% 46% at 70% 48%,rgba(255,100,59,.22),rgba(255,100,59,0) 70%),radial-gradient(30% 40% at 78% 40%,rgba(145,220,235,.08),transparent 70%)')} />}
        <div style={css('position:absolute;inset:0;z-index:1;pointer-events:none;background:radial-gradient(ellipse 80% 75% at 50% 46%,rgba(8,9,12,0) 45%,rgba(4,4,6,.82) 100%)')} />
        <div ref={grainRef} style={{ ...css('position:absolute;inset:-10%;z-index:2;pointer-events:none;opacity:.07;mix-blend-mode:overlay'), backgroundImage: grain ? `url(${grain})` : 'none' }} />
        <div ref={barTopRef} style={css('position:absolute;left:0;right:0;top:0;height:64px;background:#040406;z-index:3;transform:scaleY(0);transform-origin:top')} />
        <div ref={barBotRef} style={css('position:absolute;left:0;right:0;bottom:0;height:64px;background:#040406;z-index:3;transform:scaleY(0);transform-origin:bottom')} />
        <div
          ref={sweepRef}
          style={css(
            'position:absolute;top:0;bottom:0;left:0;width:60%;z-index:4;pointer-events:none;opacity:0;background:linear-gradient(90deg,rgba(255,100,59,0),rgba(255,100,59,.07) 40%,rgba(242,241,238,.09) 50%,rgba(145,220,235,.05) 60%,rgba(145,220,235,0))',
          )}
        />

        {/* Title bar (window drag region) */}
        <div
          data-tauri-drag-region="true"
          style={css(
            'position:absolute;top:0;left:0;right:0;height:40px;z-index:10;display:flex;align-items:center;justify-content:space-between;padding-left:18px;border-bottom:1px solid rgba(242,241,238,.045);background:linear-gradient(180deg,rgba(8,9,12,.7),rgba(8,9,12,0));user-select:none',
          )}
        >
          <div data-tauri-drag-region="true" style={css('display:flex;align-items:center;gap:12px')}>
            <div style={css('position:relative;width:20px;height:13px;flex:none')}>
              <div style={css('position:absolute;left:0;top:0;width:13px;height:13px;border-radius:7px;border:1.5px solid #FF643B')} />
              <div style={css('position:absolute;left:7px;top:0;width:13px;height:13px;border-radius:7px;border:1.5px solid #91DCEB;opacity:.85')} />
            </div>
            <span style={css("font:500 10.5px 'JetBrains Mono';letter-spacing:.24em;color:#F2F1EE")}>CROSSTALK</span>
            <span style={css("font:500 10.5px 'JetBrains Mono';letter-spacing:.24em;color:#807F84")}>SETUP</span>
          </div>
          <div data-tauri-drag-region="true" style={css('position:absolute;left:50%;top:0;height:40px;transform:translateX(-50%);display:flex;align-items:center;gap:14px')}>
            {STEPS.map((l, i) => (
              <div key={l} style={css('display:flex;align-items:center;gap:14px')}>
                <div style={{ ...css('width:22px;height:1px;transition:background .6s'), background: i <= screen ? 'rgba(255,100,59,.6)' : 'rgba(242,241,238,.12)', display: i === 0 ? 'none' : 'block' }} />
                <div style={css("display:flex;align-items:center;gap:7px;font:500 10px 'JetBrains Mono';letter-spacing:.16em")}>
                  <span style={{ color: i === screen ? A : i < screen ? '#A9A8A4' : '#5d5c61', transition: 'color .5s' }}>{'0' + (i + 1)}</span>
                  <span style={{ color: i === screen ? '#F2F1EE' : i < screen ? '#A9A8A4' : '#5d5c61', transition: 'color .5s' }}>{l}</span>
                </div>
              </div>
            ))}
          </div>
          <div style={css('display:flex;height:40px')}>
            <Btn aria-label="Réduire" onClick={() => setup.minimize()} s="width:48px;height:40px;border:none;background:transparent;color:#A9A8A4;font:400 13px 'JetBrains Mono';cursor:pointer;transition:background .2s,color .2s" hover="background:rgba(242,241,238,.06);color:#F2F1EE">
              —
            </Btn>
            <Btn aria-label="Fermer" onClick={closeWin} s="width:48px;height:40px;border:none;background:transparent;color:#A9A8A4;font:400 13px 'JetBrains Mono';cursor:pointer;transition:background .2s,color .2s" hover="background:#c42b1c;color:#fff">
              ✕
            </Btn>
          </div>
        </div>

        <div style={css('position:absolute;left:0;right:0;top:40px;bottom:0;z-index:5')}>
          {screen === 0 && (
            <div ref={screenRef} data-screen-label="01 Welcome" style={css('position:absolute;inset:0')}>
              <div data-logo="true" style={css('position:absolute;left:56px;top:34px;display:flex;align-items:center;gap:14px')}>
                <div style={css('position:relative;width:30px;height:19px;flex:none')}>
                  <div style={css('position:absolute;left:0;top:0;width:19px;height:19px;border-radius:10px;border:2px solid #FF643B;box-shadow:0 0 14px rgba(255,100,59,.45)')} />
                  <div style={css('position:absolute;left:11px;top:0;width:19px;height:19px;border-radius:10px;border:2px solid #91DCEB')} />
                </div>
                <span data-logo-text="true" style={css("font:700 15px 'Archivo';font-stretch:125%;letter-spacing:.32em;color:#F2F1EE")}>
                  CROSSTALK
                </span>
              </div>
              <div style={css('position:absolute;left:56px;bottom:48px;width:560px;display:flex;flex-direction:column;gap:22px')}>
                <Eyebrow n="01" label="FIRST CONTACT" />
                <h1 style={css("margin:0;font:800 60px/.94 'Archivo';font-stretch:112%;letter-spacing:-.012em;color:#F2F1EE;display:flex;flex-direction:column;text-shadow:0 4px 40px rgba(0,0,0,.6)")}>
                  <span style={css('display:flex;gap:.24em')}>
                    <Word>EVERY</Word>
                    <Word>
                      VOICE<span style={{ color: A }}>.</span>
                    </Word>
                  </span>
                  <span style={css('display:flex;gap:.24em')}>
                    <Word>ONE</Word>
                    <Word>
                      LANGUAGE<span style={{ color: '#91DCEB' }}>.</span>
                    </Word>
                  </span>
                </h1>
                <p data-reveal="true" style={css("margin:0;font:400 16px/1.5 'Geist';color:#A9A8A4;max-width:400px")}>
                  {info?.installed ? `Crosstalk ${info.installed} est déjà installé : cette version le remplace.` : 'Comprenez chaque call. Peu importe la langue.'}
                </p>
                <div data-reveal="true" style={css('display:flex;padding-top:6px')}>
                  <Btn onClick={() => go(1)} onMouseMove={glowMove} s={PRIMARY(150) + ";gap:20px;height:54px;padding:0 26px 0 30px;font:600 12.5px 'JetBrains Mono'"} hover={PRIMARY_HOVER} active="transform:scale(.97)">
                    INITIALISER L'INSTALLATION<span style={css("font:400 16px 'Geist'")}>→</span>
                  </Btn>
                </div>
              </div>
              <div data-reveal="true" style={css("position:absolute;right:56px;bottom:54px;display:flex;flex-direction:column;align-items:flex-end;gap:8px;font:500 10px 'JetBrains Mono';letter-spacing:.16em;color:#807F84")}>
                <div style={css('display:flex;align-items:center;gap:8px')}>
                  <div style={css('width:5px;height:5px;border-radius:3px;background:#91DCEB;box-shadow:0 0 8px #91DCEB')} />
                  <span style={{ color: '#A9A8A4' }}>COMPATIBLE</span>
                </div>
                <span>WINDOWS 10 22H2 · WINDOWS 11 · x64</span>
                <span>VERSION {info?.version ?? '…'}</span>
              </div>
            </div>
          )}

          {screen === 1 && (
            <div ref={screenRef} data-screen-label="02 Configuration" style={css('position:absolute;inset:0')}>
              <div
                data-reveal="true"
                style={css(
                  'position:absolute;left:36px;top:22px;bottom:30px;width:492px;padding:30px 32px 28px;border-radius:18px;background:linear-gradient(160deg,rgba(17,19,24,.78),rgba(10,11,14,.62));backdrop-filter:blur(24px) saturate(1.2);border:1px solid rgba(242,241,238,.08);box-shadow:inset 0 1px 0 rgba(242,241,238,.06),0 30px 80px -30px rgba(0,0,0,.8);display:flex;flex-direction:column',
                )}
              >
                <Eyebrow n="02" label="CONFIGURATION" />
                <h2 data-reveal="true" style={css("margin:14px 0 22px;font:800 36px/.95 'Archivo';font-stretch:112%;letter-spacing:-.01em;color:#F2F1EE")}>
                  PRÉPARER
                  <br />
                  L'INSTALLATION
                </h2>
                <div data-reveal="true" style={css('display:flex;flex-direction:column;gap:9px;position:relative')}>
                  <span style={css("font:500 9.5px 'JetBrains Mono';letter-spacing:.18em;color:#807F84")}>DESTINATION</span>
                  <div style={css('display:flex;gap:8px')}>
                    <div style={css('flex:1;min-width:0;height:42px;display:flex;align-items:center;gap:10px;padding:0 14px;border-radius:10px;border:1px solid rgba(242,241,238,.09);background:rgba(4,4,6,.55)')}>
                      <div style={css('width:5px;height:5px;border-radius:3px;background:#FF643B;flex:none')} />
                      <span data-testid="setup-path" title={path} style={css("font:400 12px 'JetBrains Mono';color:#F2F1EE;overflow:hidden;text-overflow:ellipsis;white-space:nowrap")}>
                        {path}
                      </span>
                    </div>
                    <Btn
                      onClick={() => setup.pickFolder(path).then((p) => p && setPath(p))}
                      disabled={!native}
                      title={native ? undefined : 'Disponible dans l’installateur Windows'}
                      s="height:42px;padding:0 16px;border-radius:10px;border:1px solid rgba(242,241,238,.12);background:rgba(242,241,238,.03);color:#F2F1EE;font:500 12.5px 'Geist';cursor:pointer;transition:background .25s,border-color .25s"
                      hover="background:rgba(242,241,238,.07);border-color:rgba(242,241,238,.2)"
                    >
                      Parcourir…
                    </Btn>
                  </div>
                </div>
                <div data-reveal="true" style={css('display:flex;align-items:flex-end;justify-content:space-between;padding:18px 0 16px')}>
                  <div style={css('display:flex;flex-direction:column;gap:6px')}>
                    <span style={css("font:500 9.5px 'JetBrains Mono';letter-spacing:.18em;color:#807F84")}>ESPACE REQUIS</span>
                    <span data-testid="setup-required" style={css("font:700 26px/1 'Archivo';font-stretch:112%;color:#F2F1EE")}>
                      {required != null ? size(required) : '…'}
                    </span>
                  </div>
                  <div style={css('display:flex;flex-direction:column;align-items:flex-end;gap:7px')}>
                    <span style={css("padding:4px 8px;border-radius:5px;border:1px solid rgba(145,220,235,.3);font:500 9px 'JetBrains Mono';letter-spacing:.16em;color:#91DCEB")}>
                      {info && info.modelBytes > 0 ? 'MODÈLES IA INCLUS' : 'MODÈLES DÉJÀ PRÉSENTS'}
                    </span>
                    <span style={{ ...css("font:400 10.5px 'JetBrains Mono'"), color: enough ? '#807F84' : '#ff8a7a' }}>{free != null ? `Disponible : ${size(free)}${enough ? '' : ' · insuffisant'}` : 'Disponible : …'}</span>
                  </div>
                </div>
                <div data-reveal="true" style={css('display:flex;flex-direction:column')}>
                  {(
                    [
                      ['Créer un raccourci sur le bureau', desk, setDesk],
                      ['Lancer Crosstalk après l\'installation', launchAfter, setLaunchAfter],
                    ] as const
                  ).map(([label, on, set]) => {
                    const t = sw(on);
                    return (
                      <button
                        key={label}
                        role="switch"
                        aria-checked={on}
                        onClick={() => set(!on)}
                        style={css('display:flex;align-items:center;gap:14px;width:100%;padding:13px 0;background:none;border:none;border-top:1px solid rgba(242,241,238,.07);cursor:pointer;text-align:left')}
                      >
                        <span style={css("flex:1;font:400 13.5px 'Geist';color:#F2F1EE")}>{label}</span>
                        <div style={{ ...css('width:40px;height:22px;border-radius:11px;position:relative;flex:none;transition:background .35s,border-color .35s'), background: t.track, border: `1px solid ${t.border}` }}>
                          <div
                            style={{
                              ...css('position:absolute;top:2px;left:2px;width:16px;height:16px;border-radius:8px;transition:transform .55s cubic-bezier(.34,1.56,.64,1),background .3s,box-shadow .3s'),
                              background: t.knob,
                              boxShadow: t.knobGlow,
                              transform: `translateX(${on ? 18 : 0}px)`,
                            }}
                          />
                        </div>
                      </button>
                    );
                  })}
                </div>
                <div style={{ flex: 1 }} />
                <div data-reveal="true" style={css('display:flex;align-items:center;justify-content:space-between;gap:12px')}>
                  <Btn onClick={() => go(0)} s={GHOST + ";height:50px;padding:0 20px;font:500 12px 'JetBrains Mono'"} hover={GHOST_HOVER}>
                    ← RETOUR
                  </Btn>
                  <Btn onClick={startInstall} disabled={!enough || !path} onMouseMove={glowMove} s={PRIMARY(140) + ";gap:16px;height:50px;padding:0 22px 0 26px;font:600 12px 'JetBrains Mono'" + (enough ? '' : ';opacity:.45;cursor:not-allowed')} hover={enough ? PRIMARY_HOVER : undefined} active="transform:scale(.97)">
                    INSTALLER CROSSTALK<span style={css("font:400 15px 'Geist'")}>→</span>
                  </Btn>
                </div>
                <p data-reveal="true" style={css("margin:14px 0 0;font:400 11.5px/1.4 'Geist';color:#807F84")}>
                  En installant, vous acceptez les{' '}
                  <button
                    onClick={() => setTermsOpen(true)}
                    style={css('padding:0;border:none;background:none;font:inherit;color:#FF643B;cursor:pointer;text-decoration:underline;text-underline-offset:3px;text-decoration-color:rgba(255,100,59,.4)')}
                  >
                    conditions d'utilisation
                  </button>
                  .
                </p>
              </div>
            </div>
          )}

          {screen === 2 && (
            <div ref={screenRef} data-screen-label="03 Installation" style={css('position:absolute;inset:0')}>
              <Eyebrow n="03" label="INSTALLATION" style="position:absolute;left:56px;top:28px" />
              <div
                data-reveal="true"
                style={{
                  ...css("position:absolute;right:56px;top:22px;display:flex;align-items:center;gap:9px;padding:7px 13px;border-radius:999px;border:1px solid rgba(242,241,238,.1);background:rgba(8,9,12,.5);backdrop-filter:blur(12px);font:500 10px 'JetBrains Mono';letter-spacing:.16em"),
                  color: statusColor,
                }}
              >
                <div style={{ ...css('width:6px;height:6px;border-radius:3px'), background: statusColor, boxShadow: `0 0 10px ${statusColor}` }} />
                {statusLabel}
              </div>
              <div style={css('position:absolute;left:56px;right:56px;bottom:36px;display:flex;flex-direction:column;gap:18px')}>
                <div data-reveal="true" style={css('display:flex;align-items:flex-end;justify-content:space-between;gap:24px')}>
                  <div style={css('display:flex;flex-direction:column;gap:8px;padding-bottom:10px;min-width:0')} aria-live="polite">
                    <span data-testid="phase-msg" style={css("font:700 24px/1.05 'Archivo';font-stretch:112%;color:#F2F1EE")}>
                      {error ? 'L’installation n’a pas abouti.' : done ? 'Installation terminée.' : PHASES[ph][1]}
                    </span>
                    <span data-testid="phase-sub" style={{ ...css("font:400 13px 'Geist'"), color: error ? '#ff8a7a' : '#A9A8A4' }}>
                      {error ?? (done ? 'Préparation de l’écran final' : phaseSub(ph, progress?.detail ?? '', desk))}
                    </span>
                  </div>
                  <div style={css('display:flex;align-items:flex-start;gap:6px;flex:none')} role="progressbar" aria-valuemin={0} aria-valuemax={100} aria-valuenow={Math.floor(prog)}>
                    <span style={css("font:300 112px/.82 'Archivo';font-stretch:100%;letter-spacing:-.04em;color:#F2F1EE;font-variant-numeric:tabular-nums;text-shadow:0 0 60px rgba(255,100,59,.25)")}>{Math.floor(prog)}</span>
                    <span style={css("font:600 22px 'JetBrains Mono';color:#FF643B;padding-top:4px")}>%</span>
                  </div>
                </div>
                <div data-reveal="true" style={css('display:flex;flex-direction:column;gap:8px')}>
                  <div style={css('display:flex;gap:3px;height:16px;align-items:flex-end')}>
                    {Array.from({ length: NT }, (_, i) => {
                      const on = i < lit,
                        lead = on && i >= lit - 1.5 && !done;
                      return (
                        <div
                          key={i}
                          style={{
                            ...css('flex:1;border-radius:1px;transition:background .25s,height .35s cubic-bezier(.34,1.56,.64,1)'),
                            height: lead ? 16 : on ? 10 : 6,
                            background: lead ? '#ffd2c2' : on ? A : 'rgba(242,241,238,.09)',
                            boxShadow: lead ? '0 0 14px rgba(255,100,59,.9)' : 'none',
                          }}
                        />
                      );
                    })}
                  </div>
                  <div style={css('height:1px;background:rgba(242,241,238,.08);position:relative')}>
                    <div style={{ ...css('position:absolute;left:0;top:0;height:1px;background:linear-gradient(90deg,rgba(255,100,59,.2),#FF643B);transition:width .12s linear'), width: prog + '%' }} />
                  </div>
                </div>
                <div data-reveal="true" style={css('display:flex;align-items:center;gap:0')}>
                  {PHASES.map((p, i) => {
                    const pd = i < ph || done,
                      cur = i === ph && !done;
                    return (
                      <div key={p[0]} style={css('flex:1;display:flex;align-items:center;gap:9px')}>
                        <div style={{ ...css('width:6px;height:6px;border-radius:3px;transition:all .4s'), border: `1px solid ${pd || cur ? A : 'rgba(242,241,238,.2)'}`, background: pd ? A : 'transparent' }} />
                        <span style={{ ...css("font:500 10px 'JetBrains Mono';letter-spacing:.14em;transition:color .4s"), color: cur ? '#F2F1EE' : pd ? '#A9A8A4' : '#5d5c61' }}>{p[0]}</span>
                      </div>
                    );
                  })}
                  {error ? (
                    <Btn onClick={() => go(1)} s={GHOST + ";padding:8px 14px;border:1px solid rgba(242,241,238,.1);font:500 10.5px 'JetBrains Mono'"} hover={GHOST_HOVER}>
                      RÉESSAYER
                    </Btn>
                  ) : (
                    <Btn onClick={cancel} disabled={done || (progress?.phase ?? 0) >= 2} title={(progress?.phase ?? 0) >= 2 && !done ? "Les fichiers sont en place : l’installation se termine" : undefined} s={GHOST + ";padding:8px 14px;border:1px solid rgba(242,241,238,.1);font:500 10.5px 'JetBrains Mono'"} hover={GHOST_HOVER}>
                      ANNULER
                    </Btn>
                  )}
                </div>
              </div>
            </div>
          )}

          {screen === 3 && (
            <div ref={screenRef} data-screen-label="04 Complete" style={css('position:absolute;inset:0')}>
              <div style={css('position:absolute;left:0;right:0;bottom:34px;display:flex;flex-direction:column;align-items:center;text-align:center;gap:14px')}>
                <Eyebrow n="04" label="INSTALLATION TERMINÉE" />
                <h1 style={css("margin:0;font:800 52px/.95 'Archivo';font-stretch:112%;letter-spacing:-.012em;color:#F2F1EE;display:flex;gap:.24em;text-shadow:0 4px 40px rgba(0,0,0,.8)")}>
                  <Word>YOU'RE</Word>
                  <Word>READY</Word>
                  <Word>TO</Word>
                  <Word>
                    CONNECT<span style={{ color: A }}>.</span>
                  </Word>
                </h1>
                <p data-reveal="true" style={css("margin:0;font:400 15px/1.5 'Geist';color:#A9A8A4")}>
                  Crosstalk est installé. Votre expérience commence maintenant.
                </p>
                <div data-reveal="true" style={css('display:flex;align-items:center;gap:12px;padding-top:8px')}>
                  {launchAfter ? (
                    <>
                      <Btn onClick={launch} onMouseMove={glowMove} s={PRIMARY(160) + ";gap:18px;height:56px;padding:0 28px 0 32px;font:600 13px 'JetBrains Mono';box-shadow:0 0 0 1px rgba(255,120,80,.6),0 14px 50px -10px rgba(255,100,59,.7)"} hover="box-shadow:0 0 0 1px rgba(255,160,130,.9),0 18px 70px -8px rgba(255,100,59,.9)" active="transform:scale(.97)">
                        {launchLabel}
                        <span style={css("font:400 16px 'Geist'")}>↗</span>
                      </Btn>
                      <Btn onClick={finish} s={GHOST + ";height:56px;padding:0 22px;font:500 12px 'JetBrains Mono'"} hover={GHOST_HOVER}>
                        FERMER
                      </Btn>
                    </>
                  ) : (
                    <>
                      <Btn onClick={launch} s={GHOST + ";height:56px;padding:0 22px;color:#F2F1EE;font:500 12px 'JetBrains Mono'"} hover="background:rgba(242,241,238,.05)">
                        {launchLabel} ↗
                      </Btn>
                      <Btn onClick={finish} onMouseMove={glowMove} s={PRIMARY(160) + ";height:56px;padding:0 32px;font:600 13px 'JetBrains Mono';box-shadow:0 14px 50px -10px rgba(255,100,59,.7)"} active="transform:scale(.97)">
                        TERMINER
                      </Btn>
                    </>
                  )}
                </div>
                <button data-reveal="true" role="switch" aria-checked={autostart} onClick={() => setAutostart(!autostart)} style={css('display:flex;align-items:center;gap:12px;padding:8px 4px 0;background:none;border:none;cursor:pointer')}>
                  <div style={{ ...css('width:34px;height:19px;border-radius:10px;position:relative;flex:none;transition:background .35s,border-color .35s'), background: as.track, border: `1px solid ${as.border}` }}>
                    <div style={{ ...css('position:absolute;top:2px;left:2px;width:13px;height:13px;border-radius:7px;transition:transform .55s cubic-bezier(.34,1.56,.64,1),background .3s'), background: as.knob, transform: `translateX(${autostart ? 15 : 0}px)` }} />
                  </div>
                  <span style={css("font:400 12.5px 'Geist';color:#A9A8A4")}>Lancer automatiquement au démarrage de Windows</span>
                </button>
              </div>
            </div>
          )}
        </div>

        {termsOpen && (
          <div onClick={() => setTermsOpen(false)} style={css('position:absolute;inset:0;z-index:20;display:flex;align-items:center;justify-content:center;background:rgba(4,4,6,.62);backdrop-filter:blur(10px)')}>
            <div
              ref={modalRef}
              onClick={(e) => e.stopPropagation()}
              role="dialog"
              aria-label="Conditions d'utilisation"
              style={css('width:540px;padding:30px 32px 26px;border-radius:18px;background:rgba(17,19,24,.94);border:1px solid rgba(242,241,238,.09);box-shadow:0 40px 100px rgba(0,0,0,.8);display:flex;flex-direction:column;gap:16px')}
            >
              <div style={css("font:500 10.5px 'JetBrains Mono';letter-spacing:.22em;color:#FF643B")}>CONDITIONS D'UTILISATION</div>
              <div style={css("display:flex;flex-direction:column;gap:12px;font:400 13px/1.6 'Geist';color:#A9A8A4")}>
                <p style={{ margin: 0 }}>
                  <span style={{ color: '#F2F1EE' }}>Logiciel tiers.</span> Crosstalk n'est ni affilié à Valve, ni approuvé par Valve. Il ne lit pas la mémoire du jeu et ne modifie aucun fichier de Counter-Strike 2.
                </p>
                <p style={{ margin: 0 }}>
                  <span style={{ color: '#F2F1EE' }}>Audio.</span> La reconnaissance vocale (Parakeet, via sherpa-onnx) s'exécute sur ce PC, hors ligne : aucune voix n'est envoyée en ligne. La traduction utilise le LLM que vous choisirez au premier lancement : un fournisseur cloud avec votre propre clé (seul le texte des phrases lui est envoyé), ou Ollama / LM Studio déjà installés sur ce PC.
                </p>
                <p style={{ margin: 0 }}>
                  <span style={{ color: '#F2F1EE' }}>Données.</span> Aucun historique n'est conservé par défaut. Le modèle de reconnaissance vocale (Parakeet TDT 0.6B v3, licence CC-BY-4.0) et la détection de voix (Silero) sont téléchargés une fois depuis Hugging Face et GitHub, avec vérification SHA-256. Aucun modèle de traduction n'est téléchargé.
                </p>
              </div>
              <div style={css('display:flex;justify-content:flex-end;padding-top:6px')}>
                <Btn onClick={() => setTermsOpen(false)} s="height:42px;padding:0 20px;border-radius:999px;border:1px solid rgba(242,241,238,.14);background:transparent;color:#F2F1EE;font:500 11.5px 'JetBrains Mono';letter-spacing:.14em;cursor:pointer" hover="background:rgba(242,241,238,.06)">
                  FERMER
                </Btn>
              </div>
            </div>
          </div>
        )}

        {confirmOpen && (
          <div onClick={() => setConfirmOpen(false)} style={css('position:absolute;inset:0;z-index:20;display:flex;align-items:center;justify-content:center;background:rgba(4,4,6,.62);backdrop-filter:blur(10px)')}>
            <div
              ref={modalRef}
              onClick={(e) => e.stopPropagation()}
              role="alertdialog"
              aria-label="Quitter l'installation"
              style={css('width:400px;padding:28px 30px 24px;border-radius:18px;background:rgba(17,19,24,.94);border:1px solid rgba(242,241,238,.09);box-shadow:0 40px 100px rgba(0,0,0,.8);display:flex;flex-direction:column;gap:12px')}
            >
              <div style={css("font:800 24px/1 'Archivo';font-stretch:112%;color:#F2F1EE")}>QUITTER L'INSTALLATION ?</div>
              <p style={css("margin:0;font:400 13px/1.55 'Geist';color:#A9A8A4")}>Crosstalk n'est pas encore installé. Vous pourrez relancer l'installation à tout moment.</p>
              <div style={css('display:flex;justify-content:flex-end;gap:8px;padding-top:10px')}>
                <Btn onClick={quit} s="height:42px;padding:0 18px;border-radius:999px;border:1px solid rgba(242,241,238,.12);background:transparent;color:#A9A8A4;font:500 11.5px 'JetBrains Mono';letter-spacing:.14em;cursor:pointer" hover="color:#F2F1EE;background:rgba(242,241,238,.05)">
                  QUITTER
                </Btn>
                <Btn onClick={() => setConfirmOpen(false)} s="height:42px;padding:0 20px;border-radius:999px;border:none;background:#FF643B;color:#08090C;font:600 11.5px 'JetBrains Mono';letter-spacing:.14em;cursor:pointer">
                  CONTINUER
                </Btn>
              </div>
            </div>
          </div>
        )}

        <div
          style={{
            ...css('position:absolute;inset:0;z-index:30;display:flex;flex-direction:column;align-items:center;justify-content:center;gap:20px;background:#040406;transition:opacity 1.2s ease'),
            opacity: ended ? 1 : 0,
            pointerEvents: ended ? 'auto' : 'none',
          }}
        >
          <div style={css('display:flex;align-items:center;gap:18px')}>
            <div style={css('position:relative;width:40px;height:25px;flex:none')}>
              <div style={css('position:absolute;left:0;top:0;width:25px;height:25px;border-radius:13px;border:2px solid #FF643B;box-shadow:0 0 20px rgba(255,100,59,.5)')} />
              <div style={css('position:absolute;left:15px;top:0;width:25px;height:25px;border-radius:13px;border:2px solid #91DCEB')} />
            </div>
            <span style={css("font:700 26px 'Archivo';font-stretch:125%;letter-spacing:.32em;color:#F2F1EE")}>CROSSTALK</span>
          </div>
          <p style={css("margin:0;font:400 13px 'Geist';color:#A9A8A4")}>{endSub}</p>
        </div>
      </div>
    </div>
  );
}


