// "Crosstalk Login" (Claude Design), shown before the app. Markup and styles
// follow the design file; the credentials are checked on this PC (auth.ts).

import { useEffect, useRef, useState, type CSSProperties, type FormEvent, type MouseEvent, type ReactNode } from 'react';
import { isTauri, windowControls } from '../../shared/ipc';
import { css } from '../../shared/ui/css';
import '@fontsource-variable/archivo/wdth.css';
import { checkCredentials, displayName, useAuth } from './auth';
import type { LoginFx, LoginPhase, LoginStage } from './stage';

const EASE = 'cubic-bezier(.16,1,.3,1)';
const A = '#FF643B';
const DOMAIN = 'crosstalk.gg';
/** Time the "VÉRIFICATION…" state lasts, as in the design. */
export const CHECK_MS = 1700;
/** Welcome screen: the three checks tick, then the app opens. */
export const ENTER_MS = 700 + 3 * 550 + 700;

const CHECKS: [string, string][] = [
  ['Profil de traduction', 'SYNCHRONISÉ'],
  ['Langues préférées', 'CHARGÉES'],
  ['Overlay en jeu', 'PRÊT'],
];

const reduced = () => typeof matchMedia === 'function' && matchMedia('(prefers-reduced-motion: reduce)').matches;
// Web Animations are missing in some test environments: animations are decoration only.
const anim = (el: Element | null | undefined, frames: Keyframe[], opts: KeyframeAnimationOptions) => {
  if (el && typeof (el as HTMLElement).animate === 'function') (el as HTMLElement).animate(frames, opts);
};

/** A design element with its hover / focus / active style variants. */
function useStates(hover?: string, focus?: string, active?: string) {
  const [h, setH] = useState(false);
  const [f, setF] = useState(false);
  const [a, setA] = useState(false);
  const extra: CSSProperties = { ...(h && hover ? css(hover) : {}), ...(f && focus ? css(focus) : {}), ...(a && active ? css(active) : {}) };
  const on = {
    onMouseEnter: () => setH(true),
    onMouseLeave: () => {
      setH(false);
      setA(false);
    },
    onMouseDown: () => setA(true),
    onMouseUp: () => setA(false),
  };
  return { extra, on, setF };
}

function Btn({ s, hover, active, children, ...rest }: { s: string; hover?: string; active?: string; children?: ReactNode } & Omit<React.ButtonHTMLAttributes<HTMLButtonElement>, 'style'>) {
  const { extra, on } = useStates(hover, undefined, active);
  return (
    <button {...rest} {...on} onMouseMove={rest.onMouseMove} style={{ ...css(s), ...extra }}>
      {children}
    </button>
  );
}

function Link({ s, hover, children, onClick }: { s: string; hover: string; children: ReactNode; onClick: (e: MouseEvent) => void }) {
  const { extra, on } = useStates(hover);
  return (
    <a href="#" {...on} onClick={onClick} style={{ ...css(s), ...extra }}>
      {children}
    </a>
  );
}

// Whole `border` (not border-color): React warns when a shorthand and its longhand mix.
const INPUT_FOCUS = 'border:1px solid rgba(255,100,59,.65);box-shadow:0 0 0 3px rgba(255,100,59,.1)';

function Field(props: React.InputHTMLAttributes<HTMLInputElement> & { s: string; onFocusChange: (f: boolean) => void }) {
  const { s, onFocusChange, ...rest } = props;
  const { extra, setF } = useStates(undefined, INPUT_FOCUS);
  return (
    <input
      {...rest}
      onFocus={() => {
        setF(true);
        onFocusChange(true);
      }}
      onBlur={() => {
        setF(false);
        onFocusChange(false);
      }}
      style={{ ...css(s), ...extra }}
    />
  );
}

export function LoginPage() {
  const login = useAuth((s) => s.login);
  const [user, setUser] = useState('');
  const [pw, setPw] = useState('');
  const [showPw, setShowPw] = useState(false);
  const [remember, setRemember] = useState(true);
  const [phase, setPhase] = useState<LoginPhase>('idle');
  const [focus, setFocus] = useState(false);
  const [caps, setCaps] = useState(false);
  const [error, setError] = useState('');
  const [bad, setBad] = useState<{ user?: boolean; pw?: boolean }>({});
  const [who, setWho] = useState('');
  const [checks, setChecks] = useState(0);
  const [no3d, setNo3d] = useState(false);
  const [grain, setGrain] = useState<string | null>(null);
  const [soon, setSoon] = useState(false);

  const stageRef = useRef<HTMLDivElement>(null);
  const heroRef = useRef<HTMLDivElement>(null);
  const cardRef = useRef<HTMLDivElement>(null);
  const okRef = useRef<HTMLDivElement>(null);
  const grainRef = useRef<HTMLDivElement>(null);
  const flashRef = useRef<HTMLDivElement>(null);
  const barTopRef = useRef<HTMLDivElement>(null);
  const barBotRef = useRef<HTMLDivElement>(null);
  const timers = useRef<ReturnType<typeof setTimeout>[]>([]);
  const fx = useRef<LoginFx>({ err: 0, flare: 0, wave: -1 });
  // What the 3D stage reads each frame.
  const live = useRef({ phase, focus, mx: 0, my: 0, introStart: performance.now(), reducedMotion: reduced() });
  live.current.phase = phase;
  live.current.focus = focus;

  const later = (fn: () => void, ms: number) => timers.current.push(setTimeout(fn, ms));

  const revealHero = () => {
    if (reduced()) return;
    heroRef.current?.querySelectorAll('[data-word]').forEach((n, k) => anim(n, [{ transform: 'translateY(108%) skewY(4deg)' }, { transform: 'none' }], { duration: 1000, delay: 100 + k * 90, easing: EASE, fill: 'both' }));
  };
  const revealCard = (base: number) => {
    const rm = reduced();
    cardRef.current?.querySelectorAll('[data-reveal]').forEach((n, i) =>
      anim(n, rm ? [{ opacity: 0 }, { opacity: 1 }] : [{ opacity: 0, transform: 'translateY(16px)', filter: 'blur(8px)' }, { opacity: 1, transform: 'none', filter: 'blur(0)' }], {
        duration: 900,
        delay: base + i * 70,
        easing: EASE,
        fill: 'both',
      }),
    );
  };
  const shakeCard = () => {
    if (!reduced()) anim(cardRef.current, [{ transform: 'translateX(0)' }, { transform: 'translateX(-9px)' }, { transform: 'translateX(7px)' }, { transform: 'translateX(-4px)' }, { transform: 'translateX(2px)' }, { transform: 'none' }], { duration: 480, easing: 'ease-out' });
  };

  // Film grain, intro and 3D stage.
  useEffect(() => {
    const c = document.createElement('canvas');
    c.width = c.height = 160;
    const g = c.getContext?.('2d');
    if (g) {
      const d = g.createImageData(160, 160);
      for (let i = 0; i < d.data.length; i += 4) {
        const v = Math.random() * 255;
        d.data[i] = d.data[i + 1] = d.data[i + 2] = v;
        d.data[i + 3] = 255;
      }
      g.putImageData(d, 0, 0);
      setGrain(c.toDataURL());
    }
    const rm = reduced();
    requestAnimationFrame(() => {
      if (!rm) anim(grainRef.current, [{ transform: 'translate(0,0)' }, { transform: 'translate(-4%,3%)' }, { transform: 'translate(3%,-2%)' }, { transform: 'translate(0,0)' }], { duration: 800, iterations: Infinity, easing: 'steps(4)' });
    });
    if (!rm) {
      [barTopRef.current, barBotRef.current].forEach((b) => anim(b, [{ transform: 'scaleY(1)' }, { transform: 'scaleY(1)', offset: 0.5 }, { transform: 'scaleY(0)' }], { duration: 2600, easing: 'cubic-bezier(.7,0,.2,1)' }));
      anim(cardRef.current, [{ opacity: 0, transform: 'translateX(30px)', filter: 'blur(12px)' }, { opacity: 1, transform: 'none', filter: 'blur(0)' }], { duration: 1300, delay: 1200, easing: EASE, fill: 'both' });
      heroRef.current?.querySelectorAll('[data-reveal]').forEach((n) => anim(n, [{ opacity: 0, transform: 'translateY(12px)' }, { opacity: 1, transform: 'none' }], { duration: 900, delay: 1700, easing: EASE, fill: 'both' }));
      heroRef.current?.querySelectorAll('[data-word]').forEach((n, k) => anim(n, [{ transform: 'translateY(108%) skewY(4deg)' }, { transform: 'none' }], { duration: 1150, delay: 1800 + k * 100, easing: EASE, fill: 'both' }));
    }
    revealCard(rm ? 0 : 1500);

    let stage: LoginStage | null = null;
    let dead = false;
    import('./stage')
      .then(({ createLoginStage }) => {
        if (dead || !stageRef.current) return;
        stage = createLoginStage(stageRef.current, () => live.current, fx.current);
        if (!stage) setNo3d(true);
      })
      .catch(() => !dead && setNo3d(true));
    const pending = timers.current;
    return () => {
      dead = true;
      stage?.dispose();
      pending.forEach(clearTimeout);
    };
    // Once, on mount: the design's componentDidMount.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // Welcome screen entrance.
  const prevPhase = useRef<LoginPhase>('idle');
  useEffect(() => {
    const prev = prevPhase.current;
    prevPhase.current = phase;
    if (prev !== 'success' && phase === 'success') {
      if (!reduced())
        okRef.current?.querySelectorAll(':scope > *').forEach((n, i) => anim(n, [{ opacity: 0, transform: 'translateY(14px)', filter: 'blur(8px)' }, { opacity: 1, transform: 'none', filter: 'blur(0)' }], { duration: 800, delay: 120 + i * 80, easing: EASE, fill: 'both' }));
      revealHero();
    }
    if (prev === 'success' && phase === 'idle') {
      revealHero();
      requestAnimationFrame(() => revealCard(0));
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [phase]);

  const submit = (e?: FormEvent) => {
    e?.preventDefault();
    if (phase === 'loading' || phase === 'success') return;
    const missing = { user: !user.trim(), pw: !pw };
    if (missing.user || missing.pw) {
      fx.current.err = 1;
      shakeCard();
      setBad(missing);
      setError(missing.user ? 'Saisissez votre identifiant ou e-mail.' : 'Saisissez votre mot de passe.');
      return;
    }
    setPhase('loading');
    setError('');
    setBad({});
    later(() => {
      const account = checkCredentials(user, pw);
      if (!account) {
        fx.current.err = 1;
        shakeCard();
        setPhase('idle');
        setError('Identifiant ou mot de passe incorrect.');
        setBad({ pw: true });
        return;
      }
      fx.current.flare = 1;
      fx.current.wave = 0;
      if (!reduced()) anim(flashRef.current, [{ opacity: 0 }, { opacity: 1, offset: 0.15 }, { opacity: 0 }], { duration: 1600, easing: 'ease-out' });
      setWho(displayName(user));
      setChecks(0);
      setPhase('success');
      [1, 2, 3].forEach((n) => later(() => setChecks(n), 700 + n * 550));
      later(() => login(account, remember), ENTER_MS);
    }, CHECK_MS);
  };

  const logout = () => {
    timers.current.forEach(clearTimeout);
    timers.current = [];
    setPhase('idle');
    setPw('');
    setWho('');
    setChecks(0);
    setError('');
    setBad({});
  };

  // No crosstalk.gg site yet: say so instead of opening nothing.
  const openSite = (e: MouseEvent) => {
    e.preventDefault();
    setSoon(true);
  };

  const ok = phase === 'success',
    load = phase === 'loading';
  const red = 'rgba(255,100,59,.75)',
    norm = 'rgba(242,241,238,.09)';
  const eyebrowColor = ok ? '#91DCEB' : A;
  const cardBorder = ok ? 'rgba(145,220,235,.22)' : error ? 'rgba(255,100,59,.25)' : 'rgba(242,241,238,.08)';

  return (
    <div
      className={`app-shell ${isTauri ? 'native' : 'framed'}`}
      data-testid="login"
      onMouseMove={(e) => {
        const r = e.currentTarget.getBoundingClientRect();
        live.current.mx = ((e.clientX - r.left) / r.width) * 2 - 1;
        live.current.my = ((e.clientY - r.top) / r.height) * 2 - 1;
      }}
      style={{ background: '#08090C' }}
    >
      <div data-screen-label="Crosstalk Login" role="dialog" aria-label="Connexion à Crosstalk" style={css('position:absolute;inset:0;overflow:hidden;background:#08090C;color:#F2F1EE;font-family:\'Geist\',system-ui,sans-serif')}>
        <div ref={stageRef} style={css('position:absolute;inset:0;z-index:0')} />
        {no3d && <div style={css('position:absolute;inset:0;z-index:0;background:radial-gradient(34% 46% at 30% 40%,rgba(255,100,59,.2),rgba(255,100,59,0) 70%),radial-gradient(26% 36% at 38% 36%,rgba(145,220,235,.08),transparent 70%)')} />}
        <div style={css('position:absolute;inset:0;z-index:1;pointer-events:none;background:radial-gradient(ellipse 85% 80% at 40% 45%,rgba(8,9,12,0) 40%,rgba(4,4,6,.85) 100%),linear-gradient(90deg,rgba(8,9,12,0) 50%,rgba(8,9,12,.55) 100%)')} />
        <div ref={grainRef} style={{ ...css('position:absolute;inset:-10%;z-index:2;pointer-events:none;opacity:.06;mix-blend-mode:overlay'), backgroundImage: grain ? `url(${grain})` : 'none' }} />
        <div ref={flashRef} style={css('position:absolute;inset:0;z-index:3;pointer-events:none;opacity:0;background:radial-gradient(50% 60% at 30% 42%,rgba(255,236,226,.35),rgba(255,100,59,.08) 50%,transparent 75%)')} />
        <div ref={barTopRef} style={css('position:absolute;left:0;right:0;top:0;height:70px;background:#040406;z-index:20;transform:scaleY(0);transform-origin:top')} />
        <div ref={barBotRef} style={css('position:absolute;left:0;right:0;bottom:0;height:70px;background:#040406;z-index:20;transform:scaleY(0);transform-origin:bottom')} />

        <div data-tauri-drag-region style={css('position:absolute;top:0;left:0;right:0;height:40px;z-index:10;display:flex;align-items:center;justify-content:space-between;padding-left:18px;border-bottom:1px solid rgba(242,241,238,.045);background:linear-gradient(180deg,rgba(8,9,12,.7),rgba(8,9,12,0));user-select:none')}>
          <div data-tauri-drag-region style={css('display:flex;align-items:center;gap:12px')}>
            <div style={css('position:relative;width:20px;height:13px;flex:none')}>
              <div style={css('position:absolute;left:0;top:0;width:13px;height:13px;border-radius:7px;border:1.5px solid #FF643B')} />
              <div style={css('position:absolute;left:7px;top:0;width:13px;height:13px;border-radius:7px;border:1.5px solid #91DCEB;opacity:.85')} />
            </div>
            <span style={css("font:500 10.5px 'JetBrains Mono';letter-spacing:.24em;color:#F2F1EE")}>CROSSTALK</span>
            <span style={css("font:500 10.5px 'JetBrains Mono';letter-spacing:.24em;color:#807F84")}>{ok ? 'SESSION' : 'CONNEXION'}</span>
          </div>
          <div style={css('display:flex;height:40px')}>
            <Btn aria-label="Réduire" onClick={() => void windowControls.minimize()} s="width:48px;height:40px;border:none;background:transparent;color:#A9A8A4;font:400 13px 'JetBrains Mono';cursor:pointer;transition:background .2s,color .2s" hover="background:rgba(242,241,238,.06);color:#F2F1EE">
              —
            </Btn>
            <Btn aria-label="Fermer" onClick={() => void windowControls.close()} s="width:48px;height:40px;border:none;background:transparent;color:#A9A8A4;font:400 13px 'JetBrains Mono';cursor:pointer;transition:background .2s,color .2s" hover="background:#c42b1c;color:#fff">
              ✕
            </Btn>
          </div>
        </div>

        <div ref={heroRef} style={css('position:absolute;left:56px;bottom:46px;width:440px;z-index:5;display:flex;flex-direction:column;gap:16px;pointer-events:none')}>
          <div data-reveal="true" style={{ ...css("display:flex;align-items:center;gap:12px;font:500 10.5px 'JetBrains Mono';letter-spacing:.22em;transition:color .5s"), color: eyebrowColor }}>
            <span>{ok ? '02' : '00'}</span>
            <div style={{ ...css('width:28px;height:1px;transition:background .5s'), background: eyebrowColor }} />
            <span>{ok ? 'SIGNAL VERROUILLÉ' : load ? 'AUTHENTIFICATION…' : 'BON RETOUR'}</span>
          </div>
          <h1 style={css("margin:0;font:800 50px/.94 'Archivo';font-stretch:112%;letter-spacing:-.012em;color:#F2F1EE;display:flex;flex-direction:column;text-shadow:0 4px 40px rgba(0,0,0,.7)")}>
            <span style={css('display:block;overflow:hidden;padding-bottom:.06em')}>
              <span data-word="true" style={css('display:inline-block')}>
                {ok ? 'VOUS ÊTES' : 'REPRENEZ'}
              </span>
            </span>
            <span style={css('display:block;overflow:hidden;padding-bottom:.06em')}>
              <span data-word="true" style={css('display:inline-block')}>
                {ok ? 'EN LIGNE' : 'LE CALL'}
                <span style={css('color:#91DCEB')}>.</span>
              </span>
            </span>
          </h1>
        </div>

        <div
          ref={cardRef}
          style={{
            ...css(
              'position:absolute;right:36px;top:62px;bottom:30px;width:384px;z-index:6;padding:30px 32px 22px;border-radius:18px;background:linear-gradient(160deg,rgba(17,19,24,.8),rgba(10,11,14,.64));backdrop-filter:blur(26px) saturate(1.2);-webkit-backdrop-filter:blur(26px) saturate(1.2);box-shadow:inset 0 1px 0 rgba(242,241,238,.06),0 30px 80px -30px rgba(0,0,0,.85);display:flex;flex-direction:column;transition:border-color .6s',
            ),
            border: `1px solid ${cardBorder}`,
          }}
        >
          {!ok && (
            <form onSubmit={submit} noValidate style={css('display:flex;flex-direction:column;flex:1;min-height:0')}>
              <div data-reveal="true" style={css("display:flex;align-items:center;gap:12px;font:500 10.5px 'JetBrains Mono';letter-spacing:.22em;color:#FF643B")}>
                <span>01</span>
                <div style={css('width:28px;height:1px;background:#FF643B')} />
                <span>CONNEXION</span>
              </div>
              <h2 data-reveal="true" style={css("margin:14px 0 10px;font:800 34px/.95 'Archivo';font-stretch:112%;letter-spacing:-.01em;color:#F2F1EE")}>
                IDENTIFIEZ-
                <br />
                VOUS
              </h2>
              <p data-reveal="true" style={css("margin:0 0 22px;font:400 13px/1.5 'Geist';color:#A9A8A4;text-wrap:pretty")}>
                Utilisez les identifiants de votre compte <span style={css('color:#F2F1EE')}>{DOMAIN}</span>.
              </p>

              <div style={css('display:flex;flex-direction:column;gap:14px')}>
                <label data-reveal="true" style={css('display:flex;flex-direction:column;gap:7px')}>
                  <span style={{ ...css("font:500 9.5px 'JetBrains Mono';letter-spacing:.18em;transition:color .3s"), color: bad.user ? A : '#807F84' }}>IDENTIFIANT OU E-MAIL</span>
                  <Field
                    value={user}
                    onChange={(e) => {
                      setUser(e.target.value);
                      setBad((b) => ({ ...b, user: false }));
                      setError('');
                    }}
                    onFocusChange={setFocus}
                    disabled={load}
                    placeholder="pseudo ou vous@exemple.com"
                    autoComplete="username"
                    aria-invalid={bad.user ? 'true' : 'false'}
                    s={`height:44px;padding:0 14px;border-radius:10px;border:1px solid ${bad.user ? red : norm};background:rgba(4,4,6,.6);color:#F2F1EE;font:400 13.5px 'Geist';outline:none;transition:border-color .25s,box-shadow .25s`}
                  />
                </label>
                <label data-reveal="true" style={css('display:flex;flex-direction:column;gap:7px')}>
                  <span style={{ ...css("font:500 9.5px 'JetBrains Mono';letter-spacing:.18em;transition:color .3s"), color: bad.pw ? A : '#807F84' }}>MOT DE PASSE</span>
                  <div style={css('position:relative;display:flex')}>
                    <Field
                      type={showPw ? 'text' : 'password'}
                      value={pw}
                      onChange={(e) => {
                        setPw(e.target.value);
                        setBad((b) => ({ ...b, pw: false }));
                        setError('');
                      }}
                      onFocusChange={setFocus}
                      onKeyUp={(e) => {
                        const c = !!e.getModifierState?.('CapsLock');
                        if (c !== caps) setCaps(c);
                      }}
                      disabled={load}
                      placeholder="••••••••"
                      autoComplete="current-password"
                      aria-invalid={bad.pw ? 'true' : 'false'}
                      s={`flex:1;min-width:0;height:44px;padding:0 74px 0 14px;border-radius:10px;border:1px solid ${bad.pw ? red : norm};background:rgba(4,4,6,.6);color:#F2F1EE;font:400 13.5px 'Geist';outline:none;transition:border-color .25s,box-shadow .25s`}
                    />
                    <Btn
                      type="button"
                      onClick={() => setShowPw((v) => !v)}
                      aria-label={showPw ? 'Masquer le mot de passe' : 'Afficher le mot de passe'}
                      s="position:absolute;right:7px;top:7px;height:30px;padding:0 10px;border-radius:7px;border:none;background:transparent;color:#807F84;font:500 9.5px 'JetBrains Mono';letter-spacing:.14em;cursor:pointer;transition:color .2s,background .2s"
                      hover="color:#F2F1EE;background:rgba(242,241,238,.05)"
                    >
                      {showPw ? 'MASQUER' : 'AFFICHER'}
                    </Btn>
                  </div>
                  {caps && <span style={css("font:500 9.5px 'JetBrains Mono';letter-spacing:.14em;color:#91DCEB")}>VERR. MAJ ACTIVÉ</span>}
                </label>
              </div>

              {error && (
                <div role="alert" style={css("display:flex;align-items:center;gap:10px;margin-top:14px;padding:10px 12px;border-radius:10px;background:rgba(255,100,59,.08);border:1px solid rgba(255,100,59,.32);font:400 12.5px/1.4 'Geist';color:#F2F1EE")}>
                  <div style={css('width:6px;height:6px;border-radius:3px;background:#FF643B;flex:none;box-shadow:0 0 8px #FF643B')} />
                  {error}
                </div>
              )}

              <div data-reveal="true" style={css('display:flex;align-items:center;justify-content:space-between;gap:10px;margin-top:16px')}>
                <button type="button" role="switch" aria-checked={remember} onClick={() => setRemember((r) => !r)} style={css('display:flex;align-items:center;gap:10px;padding:2px 0;background:none;border:none;cursor:pointer;text-align:left')}>
                  <div style={{ ...css('width:32px;height:18px;border-radius:9px;position:relative;flex:none;transition:background .35s,border-color .35s'), background: remember ? 'rgba(255,100,59,.22)' : 'rgba(242,241,238,.05)', border: `1px solid ${remember ? 'rgba(255,100,59,.6)' : 'rgba(242,241,238,.14)'}` }}>
                    <div style={{ ...css('position:absolute;top:2px;left:2px;width:12px;height:12px;border-radius:6px;transition:transform .55s cubic-bezier(.34,1.56,.64,1),background .3s'), background: remember ? A : '#807F84', transform: `translateX(${remember ? '14px' : '0px'})` }} />
                  </div>
                  <span style={css("font:400 12.5px 'Geist';color:#A9A8A4")}>Rester connecté</span>
                </button>
                <Link onClick={openSite} s="font:400 12.5px 'Geist';color:#A9A8A4;transition:color .2s" hover="color:#FF643B">
                  Mot de passe oublié ↗
                </Link>
              </div>
              {soon && <span style={css("margin-top:10px;font:400 11.5px/1.4 'Geist';color:#807F84")}>Les comptes {DOMAIN} arrivent bientôt.</span>}

              <div style={css('flex:1;min-height:14px')} />
              <Btn
                data-reveal="true"
                type="submit"
                onMouseMove={(e) => {
                  const t = e.currentTarget,
                    r = t.getBoundingClientRect();
                  t.style.setProperty('--mx', ((e.clientX - r.left) / r.width) * 100 + '%');
                  t.style.setProperty('--my', ((e.clientY - r.top) / r.height) * 100 + '%');
                }}
                s="position:relative;overflow:hidden;display:flex;align-items:center;justify-content:center;gap:16px;height:54px;border:none;border-radius:999px;cursor:pointer;background:radial-gradient(circle 150px at var(--mx,50%) var(--my,50%),rgba(255,236,226,.5),rgba(255,236,226,0) 70%),#FF643B;color:#08090C;font:600 12.5px 'JetBrains Mono';letter-spacing:.16em;box-shadow:0 0 0 1px rgba(255,120,80,.6),0 12px 40px -10px rgba(255,100,59,.6);transition:box-shadow .45s,transform .3s cubic-bezier(.34,1.56,.64,1)"
                hover="box-shadow:0 0 0 1px rgba(255,160,130,.9),0 16px 56px -8px rgba(255,100,59,.8)"
                active="transform:scale(.97)"
              >
                <div style={{ ...css('position:absolute;left:0;top:0;bottom:0;background:rgba(8,9,12,.14);transition:width 1.6s cubic-bezier(.4,0,.2,1)'), width: load ? '100%' : '0%' }} />
                <span style={css('position:relative')}>{load ? 'VÉRIFICATION…' : 'SE CONNECTER'}</span>
                <span style={css("position:relative;font:400 15px 'Geist'")}>{load ? '' : '→'}</span>
              </Btn>
              <div data-reveal="true" style={css("display:flex;align-items:center;justify-content:center;gap:6px;margin-top:16px;font:400 12.5px 'Geist';color:#807F84")}>
                Pas de compte ?
                <Link onClick={openSite} s="color:#F2F1EE;text-decoration:underline;text-underline-offset:3px;text-decoration-color:rgba(255,100,59,.5)" hover="color:#FF643B">
                  S'inscrire sur {DOMAIN} ↗
                </Link>
              </div>
            </form>
          )}

          {ok && (
            <div ref={okRef} data-testid="login-success" style={css('display:flex;flex-direction:column;flex:1')}>
              <div style={css("display:flex;align-items:center;gap:12px;font:500 10.5px 'JetBrains Mono';letter-spacing:.22em;color:#91DCEB")}>
                <span>02</span>
                <div style={css('width:28px;height:1px;background:#91DCEB')} />
                <span>ACCÈS AUTORISÉ</span>
              </div>
              <h2 style={css("margin:14px 0 10px;font:800 34px/.95 'Archivo';font-stretch:112%;letter-spacing:-.01em;color:#F2F1EE")}>
                BIENVENUE,
                <br />
                <span style={css('color:#FF643B')}>{who}</span>
              </h2>
              <p style={css("margin:0;font:400 13px/1.5 'Geist';color:#A9A8A4")}>Session ouverte. Chargement de votre configuration…</p>
              <div style={css('display:flex;flex-direction:column;margin-top:26px')}>
                {CHECKS.map(([label, done], i) => (
                  <div key={label} style={{ ...css('display:flex;align-items:center;justify-content:space-between;padding:12px 0;border-top:1px solid rgba(242,241,238,.07);transition:opacity .5s'), opacity: checks > i ? 1 : 0.45 }}>
                    <span style={css("font:400 13px 'Geist';color:#F2F1EE")}>{label}</span>
                    <span style={{ ...css("font:500 9.5px 'JetBrains Mono';letter-spacing:.16em"), color: checks > i ? '#91DCEB' : '#5d5c61' }}>{checks > i ? done : '…'}</span>
                  </div>
                ))}
              </div>
              <div style={css('flex:1')} />
              <Btn onClick={logout} s="height:46px;border-radius:999px;border:1px solid rgba(242,241,238,.12);background:transparent;color:#A9A8A4;font:500 11.5px 'JetBrains Mono';letter-spacing:.14em;cursor:pointer;transition:background .25s,color .25s" hover="background:rgba(242,241,238,.05);color:#F2F1EE">
                SE DÉCONNECTER
              </Btn>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
