// "Crosstalk Login v3" (Claude Design), shown before the app. Markup and
// styles follow the design file; the credentials are checked on this PC (auth.ts).

import { useEffect, useRef, useState, type CSSProperties, type FormEvent, type MouseEvent, type ReactNode } from 'react';
import { isTauri, windowControls } from '../../shared/ipc';
import { css } from '../../shared/ui/css';
import '@fontsource-variable/archivo/wdth.css';
import { checkCredentials, displayName, useAuth } from './auth';

const E = 'cubic-bezier(.22,1,.36,1)';
const DOMAIN = 'crosstalk.gg';
/** Time the "Connexion…" state lasts, as in the design. */
export const CHECK_MS = 1200;
/** Welcome screen: the three checks tick, then the app opens. */
export const ENTER_MS = 300 + 3 * 550 + 700;

/** Sample comms of the right panel, rotating every 3.2 s. */
const FEED = [
  { who: 'nikita', lang: 'RU → EN', src: 'Двое на шорте, один ранен', fr: 'Two on short, one is hit' },
  { who: 'jake', lang: 'EN → FR', src: 'Bomb down B, go for the retake', fr: 'Bombe posée en B, partez sur le retake' },
  { who: 'lucas', lang: 'FR → RU', src: 'Je lurk mid, tenez B', fr: 'Я в лурке на миде, держите B' },
  { who: 'mateo', lang: 'ES → EN', src: 'Tres en banana, cuidado', fr: 'Three in banana, careful' },
  { who: 'felix', lang: 'DE → FR', src: 'Einer im Pit, wenig HP', fr: 'Un dans le pit, peu de HP' },
  { who: 'kuba', lang: 'PL → EN', src: 'Rotuję na A, trzymajcie B', fr: 'Rotating A, hold B' },
];

const CHECKS: [string, string][] = [
  ['Profil de traduction', 'Synchronisé'],
  ['Langues préférées', 'Chargées'],
  ['Overlay en jeu', 'Prêt'],
];

// The design's page styles: keyframes and focus rings.
const PAGE_CSS = `
[data-testid=login] input::placeholder{color:#55565C}
[data-testid=login] input:-webkit-autofill{-webkit-text-fill-color:#EDECE8;-webkit-box-shadow:0 0 0 40px #0E0F12 inset}
[data-testid=login] :focus-visible{outline:2px solid #FF643B;outline-offset:2px}
[data-testid=login] input:focus-visible{outline:none}
@keyframes ct-spin{to{transform:rotate(360deg)}}
@keyframes ct-eq{0%,100%{transform:scaleY(.25)}50%{transform:scaleY(1)}}
@keyframes ct-pulse{0%{box-shadow:0 0 0 0 rgba(95,208,138,.55)}100%{box-shadow:0 0 0 7px rgba(95,208,138,0)}}
`;

const reduced = () => typeof matchMedia === 'function' && matchMedia('(prefers-reduced-motion: reduce)').matches;
// Web Animations are missing in some test environments: animations are decoration only.
const anim = (el: Element | null | undefined, frames: Keyframe[], opts: KeyframeAnimationOptions): Animation | null =>
  el && typeof (el as HTMLElement).animate === 'function' ? (el as HTMLElement).animate(frames, opts) : null;

/** Hover / focus / active style variants of a design element. */
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
    onFocus: () => setF(true),
    onBlur: () => setF(false),
  };
  return { extra, on };
}

function Btn({ s, hover, active, children, ...rest }: { s: string; hover?: string; active?: string; children?: ReactNode } & Omit<React.ButtonHTMLAttributes<HTMLButtonElement>, 'style'>) {
  const { extra, on } = useStates(hover, undefined, active);
  return (
    <button {...rest} {...on} style={{ ...css(s), ...extra }}>
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
const INPUT_FOCUS = 'border:1px solid #FF643B;box-shadow:0 0 0 3px rgba(255,100,59,.16)';

function Field({ s, ...rest }: React.InputHTMLAttributes<HTMLInputElement> & { s: string }) {
  const { extra, on } = useStates(undefined, INPUT_FOCUS);
  return <input {...rest} onFocus={on.onFocus} onBlur={on.onBlur} style={{ ...css(s), ...extra }} />;
}

export function LoginPage() {
  const login = useAuth((s) => s.login);
  const [user, setUser] = useState('');
  const [pw, setPw] = useState('');
  const [showPw, setShowPw] = useState(false);
  const [remember, setRemember] = useState(true);
  const [phase, setPhase] = useState<'idle' | 'loading' | 'success'>('idle');
  const [caps, setCaps] = useState(false);
  const [error, setError] = useState('');
  const [bad, setBad] = useState<{ user?: boolean; pw?: boolean }>({});
  const [who, setWho] = useState('');
  const [checks, setChecks] = useState(0);
  const [tick, setTick] = useState(0);
  const [soon, setSoon] = useState(false);

  const leftRef = useRef<HTMLDivElement>(null);
  const fieldsRef = useRef<HTMLDivElement>(null);
  const okRef = useRef<HTMLDivElement>(null);
  const gridRef = useRef<HTMLDivElement>(null);
  const panelRef = useRef<HTMLDivElement>(null);
  const feedRef = useRef<HTMLDivElement>(null);
  const timers = useRef<ReturnType<typeof setTimeout>[]>([]);
  const later = (fn: () => void, ms: number) => timers.current.push(setTimeout(fn, ms));

  const stagger = (root: HTMLElement | null, base: number, step: number, dist: number) => {
    const rm = reduced();
    root?.querySelectorAll('[data-in]').forEach((n, i) =>
      anim(n, rm ? [{ opacity: 0 }, { opacity: 1 }] : [{ opacity: 0, transform: `translateY(${dist}px)`, filter: 'blur(4px)' }, { opacity: 1, transform: 'none', filter: 'blur(0)' }], {
        duration: rm ? 200 : 700,
        delay: rm ? 0 : base + i * step,
        easing: E,
        fill: 'both',
      }),
    );
  };
  const shake = () => {
    if (!reduced()) anim(fieldsRef.current, [{ transform: 'none' }, { transform: 'translateX(-6px)' }, { transform: 'translateX(5px)' }, { transform: 'translateX(-3px)' }, { transform: 'translateX(1px)' }, { transform: 'none' }], { duration: 380, easing: 'ease-out' });
  };

  // Entrance, drifting grid, rotating feed.
  useEffect(() => {
    const rm = reduced();
    stagger(leftRef.current, 120, 60, 12);
    stagger(panelRef.current, 320, 110, 16);
    if (!rm) anim(gridRef.current, [{ backgroundPosition: '-1px -1px' }, { backgroundPosition: '39px 39px' }], { duration: 18000, iterations: Infinity });
    const iv = rm ? undefined : setInterval(() => setTick((t) => t + 1), 3200);
    const pending = timers.current;
    return () => {
      clearInterval(iv);
      pending.forEach(clearTimeout);
    };
    // Once, on mount: the design's componentDidMount.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // A new comm slides in at the top; its translation is revealed left to right.
  useEffect(() => {
    if (!tick || reduced()) return;
    const rows = feedRef.current ? [...feedRef.current.querySelectorAll('[data-row]')] : [];
    if (!rows.length) return;
    const hh = (rows[0] as HTMLElement).offsetHeight;
    rows.forEach((r, i) =>
      i === 0 ? anim(r, [{ opacity: 0, transform: 'translateY(-14px)' }, { opacity: 1, transform: 'none' }], { duration: 600, easing: E }) : anim(r, [{ transform: `translateY(-${hh}px)` }, { transform: 'none' }], { duration: 700, easing: E }),
    );
    anim(rows[0].querySelector('[data-fr]'), [{ clipPath: 'inset(0 100% 0 0)', opacity: 0.4 }, { clipPath: 'inset(0 0 0 0)', opacity: 1 }], { duration: 750, delay: 500, easing: 'cubic-bezier(.65,0,.35,1)', fill: 'both' });
  }, [tick]);

  // Welcome screen entrance, and the form coming back.
  const prevPhase = useRef(phase);
  useEffect(() => {
    const prev = prevPhase.current;
    prevPhase.current = phase;
    const rm = reduced();
    if (prev !== 'success' && phase === 'success' && okRef.current)
      [...okRef.current.children].forEach((n, i) => anim(n, rm ? [{ opacity: 0 }, { opacity: 1 }] : [{ opacity: 0, transform: 'translateY(10px)' }, { opacity: 1, transform: 'none' }], { duration: 600, delay: i * 80, easing: E, fill: 'both' }));
    if (prev === 'success' && phase === 'idle') requestAnimationFrame(() => stagger(leftRef.current, 0, 50, 10));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [phase]);

  useEffect(() => {
    if (!checks || reduced()) return;
    anim(okRef.current?.querySelectorAll('[data-check]')[checks - 1], [{ opacity: 0, transform: 'translateX(-6px)' }, { opacity: 1, transform: 'none' }], { duration: 450, easing: E });
  }, [checks]);

  useEffect(() => {
    if (error && !reduced()) anim(leftRef.current?.querySelector('[data-alert]'), [{ opacity: 0, transform: 'translateY(-6px)' }, { opacity: 1, transform: 'none' }], { duration: 400, easing: E });
  }, [error]);

  const submit = (e?: FormEvent) => {
    e?.preventDefault();
    if (phase !== 'idle') return;
    const missing = { user: !user.trim(), pw: !pw };
    if (missing.user || missing.pw) {
      shake();
      setBad(missing);
      setError('');
      return;
    }
    setPhase('loading');
    setError('');
    setBad({});
    later(() => {
      const account = checkCredentials(user, pw);
      if (!account) {
        shake();
        setPhase('idle');
        setError('Identifiant ou mot de passe incorrect. Vérifiez vos informations puis réessayez.');
        setPw('');
        return;
      }
      const name = displayName(user);
      const go = () => {
        setWho(name);
        setChecks(0);
        setPhase('success');
        [1, 2, 3].forEach((n) => later(() => setChecks(n), 300 + n * 550));
        later(() => login(account, remember), ENTER_MS);
      };
      const form = leftRef.current?.querySelector('form');
      const out = reduced() ? null : anim(form, [{ opacity: 1, transform: 'none' }, { opacity: 0, transform: 'translateY(-8px)' }], { duration: 260, easing: 'ease-in', fill: 'forwards' });
      if (out) out.onfinish = go;
      else go();
    }, CHECK_MS);
  };

  const changeAccount = () => {
    timers.current.forEach(clearTimeout);
    timers.current = [];
    setPhase('idle');
    setPw('');
    setWho('');
    setChecks(0);
    setError('');
    setBad({});
  };

  // No crosstalk.gg site yet: say so instead of opening a page that does not exist.
  const openSite = (e: MouseEvent) => {
    e.preventDefault();
    setSoon(true);
  };

  const load = phase === 'loading',
    ok = phase === 'success';
  const errB = '#B4452A',
    norm = '#26272C';
  const rm = reduced();
  const start = tick % FEED.length;
  const feed = [0, 1, 2].map((i) => ({ ...FEED[(((start - i) % FEED.length) + FEED.length) % FEED.length], op: i === 0 ? 1 : i === 1 ? 0.7 : 0.4 }));

  return (
    <div className={`app-shell ${isTauri ? 'native' : 'framed'}`} data-testid="login" style={{ background: '#0B0C0F' }}>
      <style>{PAGE_CSS}</style>
      <div data-screen-label="Crosstalk Login v3" role="dialog" aria-label="Connexion à Crosstalk" style={css("position:absolute;inset:0;overflow:hidden;background:#0B0C0F;color:#EDECE8;font-family:'Geist',system-ui,sans-serif;display:flex;flex-direction:column;-webkit-font-smoothing:antialiased")}>
        <div data-tauri-drag-region style={css('height:38px;flex:none;display:flex;align-items:center;justify-content:space-between;padding-left:16px;border-bottom:1px solid #18191D;user-select:none')}>
          <div data-tauri-drag-region style={css('display:flex;align-items:center;gap:10px')}>
            <div style={css('position:relative;width:18px;height:12px;flex:none')}>
              <div style={css('position:absolute;left:0;top:0;width:12px;height:12px;border-radius:6px;border:1.5px solid #FF643B')} />
              <div style={css('position:absolute;left:6px;top:0;width:12px;height:12px;border-radius:6px;border:1.5px solid #91DCEB')} />
            </div>
            <span style={css("font:600 12px 'Geist';letter-spacing:.01em;color:#EDECE8")}>Crosstalk</span>
          </div>
          <div style={css('display:flex;height:38px')}>
            <Btn aria-label="Réduire" onClick={() => void windowControls.minimize()} s="width:46px;height:38px;border:none;background:transparent;color:#8A8B91;font:400 12px 'Geist';cursor:pointer" hover="background:#16171B;color:#EDECE8">
              —
            </Btn>
            <Btn aria-label="Fermer" onClick={() => void windowControls.close()} s="width:46px;height:38px;border:none;background:transparent;color:#8A8B91;font:400 12px 'Geist';cursor:pointer" hover="background:#C42B1C;color:#fff">
              ✕
            </Btn>
          </div>
        </div>

        <div style={css('flex:1;min-height:0;display:grid;grid-template-columns:460px minmax(0,1fr)')}>
          <div ref={leftRef} style={css('display:flex;flex-direction:column;padding:56px 64px 28px')}>
            {!ok && (
              <form onSubmit={submit} noValidate style={css('display:flex;flex-direction:column;flex:1')}>
                <h1 data-in="true" style={css("margin:0;font:700 28px/1.1 'Archivo';font-stretch:104%;letter-spacing:-.015em;color:#EDECE8")}>
                  Connexion
                </h1>
                <p data-in="true" style={css("margin:10px 0 32px;font:400 14px/1.5 'Geist';color:#8A8B91;text-wrap:pretty")}>
                  Utilisez votre compte <span style={css('color:#EDECE8')}>{DOMAIN}</span>.
                </p>

                <div ref={fieldsRef} style={css('display:flex;flex-direction:column;gap:20px')}>
                  <label data-in="true" style={css('display:flex;flex-direction:column;gap:8px')}>
                    <span style={css("font:500 13px 'Geist';color:#C9C8C3")}>Identifiant ou e-mail</span>
                    <Field
                      value={user}
                      onChange={(e) => {
                        setUser(e.target.value);
                        setBad((b) => ({ ...b, user: false }));
                        setError('');
                      }}
                      disabled={load}
                      placeholder="vous@exemple.com"
                      autoComplete="username"
                      autoFocus
                      aria-invalid={bad.user ? 'true' : 'false'}
                      s={`height:44px;padding:0 14px;border-radius:8px;border:1px solid ${bad.user ? errB : norm};background:#0E0F12;color:#EDECE8;font:400 14px 'Geist';outline:none;transition:border-color .15s,box-shadow .15s`}
                    />
                    {bad.user && <span style={css("font:400 12.5px 'Geist';color:#FF8466")}>Saisissez votre identifiant ou e-mail.</span>}
                  </label>
                  <label data-in="true" style={css('display:flex;flex-direction:column;gap:8px')}>
                    <div style={css('display:flex;align-items:baseline;justify-content:space-between;gap:12px')}>
                      <span style={css("font:500 13px 'Geist';color:#C9C8C3")}>Mot de passe</span>
                      <Link onClick={openSite} s="font:400 13px 'Geist';color:#8A8B91" hover="color:#EDECE8">
                        Mot de passe oublié ?
                      </Link>
                    </div>
                    <div style={css('position:relative;display:flex')}>
                      <Field
                        type={showPw ? 'text' : 'password'}
                        value={pw}
                        onChange={(e) => {
                          setPw(e.target.value);
                          setBad((b) => ({ ...b, pw: false }));
                          setError('');
                        }}
                        onKeyUp={(e) => {
                          const c = !!e.getModifierState?.('CapsLock');
                          if (c !== caps) setCaps(c);
                        }}
                        disabled={load}
                        aria-label="Mot de passe"
                        autoComplete="current-password"
                        aria-invalid={bad.pw || error ? 'true' : 'false'}
                        s={`flex:1;min-width:0;height:44px;padding:0 84px 0 14px;border-radius:8px;border:1px solid ${bad.pw || error ? errB : norm};background:#0E0F12;color:#EDECE8;font:400 14px 'Geist';outline:none;transition:border-color .15s,box-shadow .15s`}
                      />
                      <Btn
                        type="button"
                        onClick={() => setShowPw((v) => !v)}
                        aria-label={showPw ? 'Masquer le mot de passe' : 'Afficher le mot de passe'}
                        s="position:absolute;right:6px;top:6px;height:32px;padding:0 10px;border-radius:6px;border:none;background:transparent;color:#8A8B91;font:500 12.5px 'Geist';cursor:pointer"
                        hover="color:#EDECE8;background:#17181C"
                      >
                        {showPw ? 'Masquer' : 'Afficher'}
                      </Btn>
                    </div>
                    {bad.pw && <span style={css("font:400 12.5px 'Geist';color:#FF8466")}>Saisissez votre mot de passe.</span>}
                    {caps && <span style={css("font:400 12.5px 'Geist';color:#91DCEB")}>Verr. Maj activé</span>}
                  </label>
                </div>

                {error && (
                  <div role="alert" data-alert="true" style={css("margin-top:20px;padding:12px 14px;border-radius:8px;background:#1A1210;border:1px solid #4A2519;font:400 13px/1.45 'Geist';color:#F3D9D0")}>
                    {error}
                  </div>
                )}

                <button
                  data-in="true"
                  type="button"
                  role="checkbox"
                  aria-checked={remember}
                  onClick={() => setRemember((r) => !r)}
                  style={css("display:flex;align-items:center;gap:10px;margin-top:22px;padding:0;background:none;border:none;cursor:pointer;color:#C9C8C3;font:400 13.5px 'Geist';align-self:flex-start")}
                >
                  <div style={{ ...css("width:18px;height:18px;border-radius:5px;display:flex;align-items:center;justify-content:center;color:#0B0C0F;font:700 11px 'Geist';transition:background .15s,border-color .15s"), border: `1px solid ${remember ? '#FF643B' : '#3A3B41'}`, background: remember ? '#FF643B' : 'transparent' }}>
                    {remember ? '✓' : ''}
                  </div>
                  Rester connecté
                </button>

                <Btn
                  data-in="true"
                  type="submit"
                  disabled={load}
                  s="margin-top:28px;height:46px;border:none;border-radius:8px;cursor:pointer;background:#FF643B;color:#140A06;font:600 14px 'Geist';display:flex;align-items:center;justify-content:center;gap:10px;transition:background .15s,transform .1s"
                  hover="background:#FF7650"
                  active="transform:translateY(1px)"
                >
                  {load && <span aria-hidden style={{ width: 14, height: 14, borderRadius: 7, border: '2px solid rgba(20,10,6,.25)', borderTopColor: '#140A06', animation: 'ct-spin .7s linear infinite', display: 'inline-block' }} />}
                  {load ? 'Connexion…' : 'Se connecter'}
                </Btn>
                {soon && <span style={css("margin-top:14px;font:400 12.5px/1.4 'Geist';color:#8A8B91")}>Les comptes {DOMAIN} arrivent bientôt.</span>}

                <div style={css('flex:1')} />
                <p data-in="true" style={css("margin:0;font:400 13px 'Geist';color:#8A8B91")}>
                  Pas encore de compte ?{' '}
                  <Link onClick={openSite} s="color:#EDECE8;text-decoration:underline;text-underline-offset:3px;text-decoration-color:#3A3B41" hover="color:#FF643B">
                    Créer un compte
                  </Link>
                </p>
              </form>
            )}

            {ok && (
              <div ref={okRef} data-testid="login-success" style={css('display:flex;flex-direction:column;flex:1')}>
                <h1 style={css("margin:0;font:700 28px/1.1 'Archivo';font-stretch:104%;letter-spacing:-.015em;color:#EDECE8")}>Bonjour, {who}</h1>
                <p style={css("margin:10px 0 28px;font:400 14px/1.5 'Geist';color:#8A8B91")}>Préparation de votre session.</p>
                <div style={css('display:flex;flex-direction:column;border-top:1px solid #1C1D22')}>
                  {CHECKS.map(([label, state], i) => {
                    const done = checks > i;
                    return (
                      <div key={label} style={css('display:flex;align-items:center;justify-content:space-between;gap:12px;padding:14px 0;border-bottom:1px solid #1C1D22')}>
                        <span style={{ ...css("font:400 14px 'Geist';transition:color .3s"), color: done ? '#EDECE8' : '#6E6F75' }}>{label}</span>
                        <span data-check="true" style={{ ...css("display:inline-block;font:500 12.5px 'Geist';transition:color .3s"), color: done ? '#5FD08A' : '#55565C' }}>
                          {done ? state : 'En attente'}
                        </span>
                      </div>
                    );
                  })}
                </div>
                <div style={css('flex:1')} />
                <Btn onClick={changeAccount} s="height:44px;border-radius:8px;border:1px solid #26272C;background:transparent;color:#C9C8C3;font:500 13.5px 'Geist';cursor:pointer" hover="background:#131417;color:#EDECE8">
                  Changer de compte
                </Btn>
              </div>
            )}
          </div>

          <div style={css('position:relative;border-left:1px solid #18191D;background:#0E0F12;display:flex;flex-direction:column;padding:56px 56px 28px;overflow:hidden')}>
            <div ref={gridRef} style={css('position:absolute;inset:0;pointer-events:none;background-image:linear-gradient(#14151A 1px,transparent 1px),linear-gradient(90deg,#14151A 1px,transparent 1px);background-size:40px 40px;background-position:-1px -1px;opacity:.6')} />
            <div ref={panelRef} style={css('position:relative;display:flex;flex-direction:column;flex:1;gap:28px')}>
              <div data-in="true" style={css('display:flex;flex-direction:column;gap:12px;max-width:380px')}>
                <span style={css("font:500 11px 'JetBrains Mono';letter-spacing:.14em;color:#FF643B")}>TRADUCTION VOCALE EN DIRECT</span>
                <p style={css("margin:0;font:600 24px/1.25 'Archivo';font-stretch:100%;letter-spacing:-.01em;color:#EDECE8;text-wrap:balance")}>Comprenez chaque call, quelle que soit la langue de vos coéquipiers.</p>
              </div>

              <div data-in="true" ref={feedRef} data-testid="login-feed" style={css('display:flex;flex-direction:column;border-radius:10px;border:1px solid #1E1F24;background:#0B0C0F;overflow:hidden')}>
                <div style={css('display:flex;align-items:center;justify-content:space-between;padding:12px 16px;border-bottom:1px solid #1A1B20')}>
                  <span style={css("display:flex;align-items:center;gap:10px;font:500 12.5px 'Geist';color:#C9C8C3")}>
                    Comms équipe
                    <span aria-hidden style={{ display: 'flex', alignItems: 'center', gap: 2, height: 12 }}>
                      {[0.9, 0.6, 1.1, 0.75, 0.95].map((d, i) => (
                        <span key={i} style={{ width: 2, height: 12, borderRadius: 1, background: '#FF643B', transformOrigin: 'center', transform: 'scaleY(.4)', animation: rm ? 'none' : `ct-eq ${d}s ease-in-out ${i * 0.12}s infinite` }} />
                      ))}
                    </span>
                  </span>
                  <span style={css("display:flex;flex:none;white-space:nowrap;align-items:center;gap:8px;font:500 11px 'JetBrains Mono';letter-spacing:.08em;color:#8A8B91")}>
                    <span style={{ width: 6, height: 6, borderRadius: 3, background: '#5FD08A', flex: 'none', animation: rm ? 'none' : 'ct-pulse 1.6s ease-out infinite' }} />
                    EN DIRECT
                  </span>
                </div>
                {feed.map((m) => (
                  <div key={m.who} data-row="true" style={{ ...css('position:relative;background:#0B0C0F;display:grid;grid-template-columns:72px minmax(0,1fr);gap:4px 14px;padding:14px 16px;border-bottom:1px solid #16171B;transition:opacity .6s'), opacity: m.op }}>
                    <span style={css("font:500 13px 'Geist';color:#EDECE8")}>{m.who}</span>
                    <span style={css("font:400 13px/1.4 'Geist';color:#6E6F75")}>
                      {m.src} <span style={css("font:500 10.5px 'JetBrains Mono';color:#55565C")}>{m.lang}</span>
                    </span>
                    <span />
                    <span data-fr="true" style={css("font:500 14px/1.4 'Geist';color:#EDECE8")}>
                      {m.fr}
                    </span>
                  </div>
                ))}
              </div>

              <div style={css('flex:1')} />
              <div data-in="true" style={css("display:flex;align-items:center;justify-content:space-between;gap:16px;font:400 12px 'Geist';color:#6E6F75")}>
                <span style={css('display:flex;align-items:center;gap:8px;white-space:nowrap;min-width:0')}>
                  <span style={css('width:6px;height:6px;border-radius:3px;background:#5FD08A;flex:none')} />
                  Tous les services opérationnels
                </span>
                <span style={css("font:400 11.5px 'JetBrains Mono';flex:none")}>v{__APP_VERSION__}</span>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
