import { useEffect, useState, type ReactNode } from 'react';
import { voice } from '../../shared/ipc';
import { Slider, ToggleRow, fnt } from '../../shared/ui/atoms';
import { useApp } from '../../app/state/store';
import { targetName, useReplyLanguages } from '../reply/ReplyBox';
import { chosenVoice, downloadPercent, megabytes, PTT_STEPS, pttView, refreshVoice, toDownload, useVoice, validPttKey, voiceSummary, voicesOf } from './voice';

const label = { font: fnt(500, 11), color: '#bdb6ab' };
const hint = { font: fnt(400, 10.5, 'sans', 1.5), color: '#6e6961' };
const body = { font: fnt(400, 11.5, 'sans', 1.5), color: '#8f8a83' };


/** The voice key is one of Crosstalk's shortcuts (Mouse 4: Répondre, Mouse 5: dictation). */
export function shortcutConflict(key: string): boolean {
  return ['MOUSE4', 'MOUSE5'].includes(key.trim().toUpperCase().replace(/\s+/g, ''));
}

function Step({ n, title, done, children }: { n: number; title: string; done?: boolean; children: ReactNode }) {
  return (
    <div className="flex flex-col gap-1.5" data-testid={`voice-step-${n}`}>
      <span style={label}>
        <span style={{ color: done ? '#7fe0a8' : '#ffa24a' }}>{done ? '✓' : `${n} ·`}</span> {title}
      </span>
      {children}
    </div>
  );
}

function Pill({ children, onClick, disabled, ember }: { children: ReactNode; onClick: () => void; disabled?: boolean; ember?: boolean }) {
  return (
    <div className={`pill ${ember ? 'pill-ember' : 'pill-ghost'} flex-none px-3.5 py-[7px]`} role="button" aria-disabled={!!disabled} onClick={() => !disabled && onClick()} style={{ font: fnt(500, 11.5) }}>
      {children}
    </div>
  );
}

/** Langues › Voix IA: your dictated reply said in the game by a local voice, through VB-CABLE. */
export function VoiceCard() {
  const opts = useApp((s) => s.voice);
  const set = useApp((s) => s.set);
  const { native, status: st, download, spoken, ptt } = useVoice();
  const { targets } = useReplyLanguages();
  const [busy, setBusy] = useState<string | null>(null);
  const [note, setNote] = useState<{ text: string; error?: boolean } | null>(null);
  useEffect(() => {
    if (native) refreshVoice();
  }, [native]);

  const setOpts = (p: Partial<typeof opts>) => set({ voice: { ...opts, ...p } });
  const langs = targets;
  const todo = st ? toDownload(st, opts, langs) : { ids: [], bytes: 0 };
  const ready = !!st && todo.bytes === 0;
  const summary = native ? voiceSummary(st, opts, ready) : { value: 'Disponible dans l’application Windows', detail: '', color: '#8f8a83' };
  const pct = downloadPercent(download);

  const run = async (key: string, f: () => Promise<string>) => {
    setBusy(key);
    try {
      setNote({ text: await f() });
    } catch (e) {
      setNote({ text: String(e), error: true });
    } finally {
      setBusy(null);
    }
  };
  const fetchVoices = () => run('download', async () => (await voice.download(todo.ids), 'Téléchargement lancé…'));
  const testInGame = (lang: string) =>
    run(`say-${lang}`, async () => {
      const ms = await voice.say(SAMPLE[lang] ?? SAMPLE.en, lang);
      return `Dite en jeu (${(ms / 1000).toFixed(1).replace('.', ',')} s). Vos coéquipiers l’entendent si CS2 écoute « CABLE Output ».`;
    });
  const testPtt = () =>
    run('ptt-test', async () => {
      const lang = langs[0] ?? 'en';
      const ms = await voice.pttTest(lang, 5);
      return `Test terminé (${(ms / 1000).toFixed(1).replace('.', ',')} s de voix).`;
    });
  const keyOk = validPttKey(opts.pttKey);
  const pv = pttView(ptt ?? st?.ptt ?? null);
  const preview = (id: string) => run(`preview-${id}`, async () => (await voice.preview(id), 'Écoute dans votre casque.'));

  return (
    <div className="glass flex flex-col gap-3.5 p-5" data-testid="voice-card">
      <div className="flex items-baseline justify-between">
        <span className="lbl">VOIX IA EN JEU</span>
        <span style={{ font: fnt(500, 9.5, 'mono'), letterSpacing: '.12em', color: '#6e6961' }}>PIPER · SUR CE PC · GRATUIT</span>
      </div>
      <span style={body}>
        Mouse 5, dites votre phrase dans votre langue : Crosstalk la traduit et une voix IA la dit dans le vocal de CS2, dans la langue de vos coéquipiers. La voix est produite sur votre PC, sans compte ni clé. En appui pour parler, Crosstalk peut maintenir votre touche vocale pendant la
        phrase (option ci-dessous, désactivée par défaut).
      </span>
      <ToggleRow title="Dire mes réponses en jeu avec une voix IA" sub="Désactivé par défaut" on={opts.enabled} onClick={() => setOpts({ enabled: !opts.enabled })} />

      <Step n={1} title="Installer le câble audio virtuel (VB-CABLE, gratuit)" done={!!st?.cable}>
        <span style={body}>
          {st?.cable ? (
            <>
              Détecté : <b style={{ color: '#ece6dc', fontWeight: 500 }}>{st.cable.name}</b>
            </>
          ) : (
            'C’est par lui que la voix arrive dans CS2. Téléchargez-le, lancez « VBCABLE_Setup_x64 » en administrateur, redémarrez le PC.'
          )}
        </span>
        <div className="flex gap-2">
          {!st?.cable && (
            <Pill ember onClick={() => voice.openCablePage()} disabled={!native}>
              Télécharger VB-CABLE ↗
            </Pill>
          )}
          <Pill onClick={() => run('check', async () => (await refreshVoice(), 'Détection relancée.'))} disabled={!native || busy === 'check'}>
            Vérifier à nouveau
          </Pill>
        </div>
      </Step>

      <Step n={2} title="Votre vrai micro" done={!!opts.mic}>
        <select className="text-input" aria-label="Micro" value={opts.mic ?? ''} disabled={!native} onChange={(e) => setOpts({ mic: e.target.value || null })}>
          <option value="">Micro par défaut de Windows</option>
          {st?.mics.map((m) => (
            <option key={m.id} value={m.id}>
              {m.name}
            </option>
          ))}
        </select>
        <span style={hint}>Choisissez votre micro (casque) : Crosstalk le fait passer dans le câble, vos coéquipiers vous entendent comme avant.</span>
      </Step>

      <Step n={3} title="Choisir les voix" done={ready && langs.length > 0}>
        {langs.length === 0 && <span style={hint}>Aucune langue de coéquipier : choisissez-les dans « Mes langues ».</span>}
        {st &&
          langs.map((lang) => {
            const list = voicesOf(st.voices, lang);
            const cur = chosenVoice(st.voices, opts, lang);
            return (
              <div key={lang} className="flex items-center gap-2" data-testid={`voice-lang-${lang}`}>
                <span className="w-[86px] flex-none" style={{ font: fnt(500, 12), color: '#ece6dc' }}>
                  {targetName(lang)}
                </span>
                <select className="text-input min-w-0 flex-1" aria-label={`Voix ${targetName(lang)}`} value={cur?.id ?? ''} onChange={(e) => setOpts({ voices: { ...opts.voices, [lang]: e.target.value } })}>
                  {list.map((v) => (
                    <option key={v.id} value={v.id}>
                      {v.name} · {v.gender === 'female' ? 'femme' : 'homme'}
                      {v.installed ? '' : ` · ${megabytes(v.onnxBytes)}`}
                    </option>
                  ))}
                </select>
                {cur?.installed ? (
                  <>
                    <Pill onClick={() => preview(cur.id)} disabled={!!busy}>
                      ▶ Écouter
                    </Pill>
                    <Pill onClick={() => testInGame(lang)} disabled={!!busy || !st.routing}>
                      Dire en jeu
                    </Pill>
                  </>
                ) : (
                  <span style={{ font: fnt(400, 10.5, 'mono'), color: '#ffa24a' }}>à télécharger</span>
                )}
              </div>
            );
          })}
        {st && todo.bytes > 0 && (
          <div className="flex items-center gap-3 rounded-xl px-3 py-2.5" style={{ background: 'rgba(255,162,74,.07)', border: '1px solid rgba(255,162,74,.22)' }}>
            <span className="flex-1" style={{ font: fnt(400, 11.5, 'sans', 1.45), color: '#ffc68a' }} data-testid="voice-download">
              {pct !== null ? `Téléchargement… ${pct} % · ${download?.current ?? ''}` : `${st.engineReady ? '' : 'Moteur Piper + '}${todo.ids.length} voix : ${megabytes(todo.bytes)}, une seule fois.`}
            </span>
            {pct === null ? (
              <Pill ember onClick={fetchVoices} disabled={!native || busy === 'download'}>
                Télécharger
              </Pill>
            ) : (
              <Pill onClick={() => voice.cancelDownload()}>Annuler</Pill>
            )}
          </div>
        )}
        {download?.state === 'error' && <span style={{ font: fnt(400, 11), color: '#ff8a7a' }}>{download.error}</span>}
      </Step>

      <Step n={4} title="Dans Windows (une fois)">
        <span style={body}>
          Paramètres › Système › Son › Entrée : choisissez <b style={{ color: '#ece6dc', fontWeight: 500 }}>CABLE Output (VB-Audio Virtual Cable)</b> comme micro par défaut. CS2 écoute alors le câble : votre voix et la voix IA. En micro ouvert, c’est
          automatique ; en appui pour parler, gardez votre touche enfoncée, ou activez l’appui automatique ci-dessous.
        </span>
      </Step>

      <div className="rounded-xl px-3.5 py-3" style={{ border: '1px solid rgba(255,255,255,.07)', background: 'rgba(255,255,255,.015)' }} data-testid="voice-typed">
        <ToggleRow
          title="Dire aussi mes réponses écrites"
          sub="Pop-up Mouse 4 : Entrée traduit, copie et fait dire la phrase par la voix IA (touche vocale appuyée si l’appui automatique est activé)"
          on={!!opts.speakTyped}
          onClick={() => setOpts({ speakTyped: !opts.speakTyped })}
        />
      </div>

      <div className="flex flex-col gap-2.5 rounded-xl px-3.5 py-3" style={{ border: '1px solid rgba(255,255,255,.07)', background: 'rgba(255,255,255,.015)' }} data-testid="voice-ptt">
        <ToggleRow
          title="Appuyer automatiquement pour parler dans CS2"
          sub="Désactivé par défaut · seulement quand CS2 est au premier plan"
          on={opts.autoPtt}
          onClick={() => setOpts({ autoPtt: !opts.autoPtt })}
        />
        {opts.autoPtt && (
          <>
            <div className="flex items-center gap-2">
              <span style={label}>Touche vocale de CS2</span>
              <input
                className="text-input w-[96px] text-center"
                aria-label="Touche vocale"
                value={opts.pttKey}
                maxLength={8}
                spellCheck={false}
                onChange={(e) => setOpts({ pttKey: e.target.value.trim().slice(0, 8) })}
                style={{ borderColor: keyOk ? undefined : 'rgba(255,138,122,.6)' }}
              />
              <span style={hint}>K, V, F5, CapsLock, Mouse3…</span>
            </div>
            {shortcutConflict(opts.pttKey) && (
              <span role="alert" data-testid="ptt-shortcut-conflict" style={{ font: fnt(400, 11, 'sans', 1.5), color: '#ffa24a' }}>
                Mouse 4 ouvre Répondre et Mouse 5 lance la dictée : avec cette touche vocale, parler brièvement ouvrirait aussi ces fenêtres. Choisissez plutôt K, V ou Mouse3.
              </span>
            )}
            {!keyOk && (
              <span role="alert" style={{ font: fnt(400, 11), color: '#ff8a7a' }}>
                Touche non reconnue : tapez une lettre, un chiffre, F1–F12, Space, CapsLock, Shift, Ctrl, Alt ou Mouse3–5.
              </span>
            )}
            <span style={hint}>
              Même touche que dans CS2 : Paramètres › Audio › « Utiliser le micro » / Touches › Communication › « Parler ». Conditions : CS2 au premier plan, « CABLE Output » choisi comme micro (étape 4), et cette touche identique à votre
              bind.
            </span>
            <div className="grid gap-3" style={{ gridTemplateColumns: '1fr 1fr' }}>
              <Slider label="Avant la phrase" unit=" ms" min={50} max={1000} value={opts.pttLeadMs} onChange={(v) => setOpts({ pttLeadMs: v })} />
              <Slider label="Après la phrase" unit=" ms" min={100} max={1500} value={opts.pttTailMs} onChange={(v) => setOpts({ pttTailMs: v })} />
            </div>
            <div className="flex items-center gap-1.5" data-testid="voice-ptt-steps">
              {PTT_STEPS.map(([name], i) => {
                const on = pv.step === i;
                const past = pv.step > i;
                return (
                  <span
                    key={name}
                    className="rounded-full px-2.5 py-[3px]"
                    data-active={on}
                    style={{
                      font: fnt(500, 10.5),
                      color: on ? '#120f0c' : past ? '#7fe0a8' : '#6e6961',
                      background: on ? '#ffa24a' : 'transparent',
                      border: `1px solid ${on ? '#ffa24a' : past ? 'rgba(127,224,168,.35)' : 'rgba(255,255,255,.08)'}`,
                    }}
                  >
                    {name}
                  </span>
                );
              })}
            </div>
            {pv.text && (
              <span style={{ font: fnt(400, 11, 'sans', 1.5), color: pv.color }} data-testid="voice-ptt-state">
                {pv.text}
              </span>
            )}
            <div className="flex items-center gap-2">
              <Pill ember onClick={testPtt} disabled={!native || !st?.routing || !keyOk || busy === 'ptt-test'}>
                Tester dans 5 s
              </Pill>
              <Pill onClick={() => void voice.cancel()} disabled={!native}>
                Annuler la transmission
              </Pill>
              <span style={hint}>Lancez le test, passez sur CS2 : la phrase d’essai part touche enfoncée.</span>
            </div>
          </>
        )}
      </div>

      <div className="grid gap-3" style={{ gridTemplateColumns: '1fr 1fr' }}>
        <Slider label="Volume de la voix IA" unit="%" min={0} max={150} value={opts.volume} onChange={(v) => setOpts({ volume: v })} />
        <Slider label="Volume de votre micro" unit="%" min={0} max={150} value={opts.micVolume} onChange={(v) => setOpts({ micVolume: v })} />
      </div>
      <ToggleRow title="M’entendre aussi dans le casque" sub="La voix IA est jouée dans votre sortie par défaut" on={opts.monitor} onClick={() => setOpts({ monitor: !opts.monitor })} />

      <div className="flex flex-col gap-1 pt-1" data-testid="voice-state">
        <span style={{ font: fnt(500, 12), color: summary.color }}>{summary.value}</span>
        {summary.detail && <span style={body}>{summary.detail}</span>}
        {spoken && opts.enabled && (
          <span style={{ font: fnt(400, 11, 'sans', 1.5), color: '#7fd4ff' }} data-testid="voice-last">
            Dernière phrase dite : « {spoken.text} »
          </span>
        )}
        {note && (
          <span role={note.error ? 'alert' : undefined} style={{ font: fnt(400, 11, 'sans', 1.5), color: note.error ? '#ff8a7a' : '#7fe0a8' }}>
            {note.text}
          </span>
        )}
      </div>

      <div className="rounded-lg px-3 py-2.5" style={{ border: '1px solid rgba(255,162,74,.22)', background: 'rgba(255,162,74,.05)', font: fnt(400, 11, 'sans', 1.5), color: '#bdb6ab' }}>
        <b style={{ color: '#ffc68a', fontWeight: 500 }}>Bon à savoir.</b> Certains serveurs ou ligues interdisent les voix de synthèse : vérifiez leur règlement. Crosstalk ne lit ni ne modifie le jeu ; la voix passe par un périphérique audio, comme
        un micro.
      </div>
    </div>
  );
}

/** Test sentence said in the game (same as the Rust preview). */
export const SAMPLE: Record<string, string> = {
  ru: 'Двое на B, осторожно, флешка.',
  en: 'Two on B, watch out for the flash.',
  fr: 'Deux en B, attention au flash.',
  de: 'Zwei auf B, Vorsicht, Blendgranate.',
  es: 'Dos en B, cuidado con la flash.',
  pl: 'Dwóch na B, uwaga na flasha.',
  tr: 'B’de iki kişi, flaşa dikkat.',
  uk: 'Двоє на B, обережно, флешка.',
};
