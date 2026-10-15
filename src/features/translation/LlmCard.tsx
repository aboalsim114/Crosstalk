import { useEffect, useState, type ReactNode } from 'react';
import { llm, LLM_DEFAULT_URL, LLM_SERVICES, llmService, serviceOf, type LlmProvider, type LlmService } from '../../shared/ipc';
import { Seg, ToggleRow, fnt } from '../../shared/ui/atoms';
import { useApp } from '../../app/state/store';
import { checkOllama, clearLlmKey, llmConfigFrom, llmStatsLine, llmSummary, MODEL_EXAMPLES, runLlmTest, saveLlmKey, serviceChange, testAdvice, useLlm } from './llm';

const PROVIDERS: [LlmProvider, string][] = [
  ['openai', 'Compatible OpenAI'],
  ['anthropic', 'Anthropic'],
];

/** Suggested first model for Ollama: small, multilingual, runs on most gaming PCs. */
export const OLLAMA_PULL = 'ollama pull qwen2.5:7b';

const label = { font: fnt(500, 11), color: '#bdb6ab' };
const hint = { font: fnt(400, 10.5, 'sans', 1.5), color: '#6e6961' };
const body = { font: fnt(400, 11, 'sans', 1.5), color: '#8f8a83' };
const btn = { font: fnt(500, 11.5) };

function Step({ n, title, children }: { n: number; title: string; children: ReactNode }) {
  return (
    <div className="flex flex-col gap-1.5">
      <span style={label}>
        <span style={{ color: '#ffa24a' }}>{n} ·</span> {title}
      </span>
      {children}
    </div>
  );
}

function Pill({ children, onClick, disabled, ember }: { children: ReactNode; onClick: () => void; disabled?: boolean; ember?: boolean }) {
  return (
    <div className={`pill ${ember ? 'pill-ember' : 'pill-ghost'} flex-none px-3.5 py-[7px]`} role="button" aria-disabled={!!disabled} onClick={() => !disabled && onClick()} style={btn}>
      {children}
    </div>
  );
}

/** Audio & IA › online translation by an LLM: pick a service, paste its key, test. */
export function LlmCard() {
  const opts = useApp((s) => s.llm);
  const setLlm = useApp((s) => s.setLlm);
  const { native, status, test, testedWith, testing, ollama, checkingOllama } = useLlm();
  const [key, setKey] = useState('');
  const [busy, setBusy] = useState(false);
  const [keyError, setKeyError] = useState<string | null>(null);
  const [advanced, setAdvanced] = useState(opts.service === 'custom');
  const info = llmService(opts.service);
  const local = info?.local ?? false;
  const config = llmConfigFrom({ llm: opts });
  const summary = !native && opts.enabled ? { value: 'État disponible dans l’application Windows', color: '#8f8a83' } : llmSummary(status, opts.enabled);
  // A result is shown only for the settings it was run with.
  const shownTest = test && testedWith === JSON.stringify(config) ? test : null;

  useEffect(() => {
    if (opts.service === 'ollama' && native && !ollama) checkOllama();
  }, [opts.service, native, ollama]);

  const pick = async (service: LlmService) => {
    if (service === 'custom') setAdvanced(true);
    if (service === 'ollama' && native) {
      setLlm(serviceChange(service, opts, ollama?.models));
      const found = await checkOllama();
      // The model list arrives after the click: the first installed model is chosen.
      const cur = useApp.getState().llm;
      if (cur.service === 'ollama' && found?.models.length && !found.models.includes(cur.model)) setLlm({ model: found.models[0] });
      return;
    }
    setLlm(serviceChange(service, opts));
  };
  const save = async () => {
    setBusy(true);
    setKeyError(await saveLlmKey(key));
    // The field is emptied either way: the key is kept by Windows, not by the interface.
    setKey('');
    setBusy(false);
  };
  const clear = async () => {
    setBusy(true);
    setKeyError(await clearLlmKey());
    setBusy(false);
  };

  return (
    <div className="glass flex flex-col gap-3 p-5" data-testid="llm-card">
      <div className="flex items-center justify-between">
        <span className="lbl">TRADUCTION EN LIGNE · LLM</span>
        <span style={{ font: fnt(500, 9.5, 'mono'), letterSpacing: '.12em', color: '#6e6961' }}>FACULTATIF</span>
      </div>
      <div style={body}>
        Une IA en ligne (ou sur votre PC) traduit les phrases plus naturellement. Choisissez un service, collez sa clé, testez : c’est tout. En cas de problème, Opus-MT traduit sur ce PC. Les lignes en direct restent toujours locales.
      </div>
      <ToggleRow title="Traduire avec un LLM en ligne" sub="Désactivé par défaut" on={opts.enabled} onClick={() => setLlm({ enabled: !opts.enabled })} />

      <Step n={1} title="Choisissez un service">
        <div className="grid grid-cols-2 gap-1.5" role="radiogroup" aria-label="Service">
          {[...LLM_SERVICES, { id: 'custom' as const, name: 'Autre', note: 'Adresse et modèle à saisir' }].map((s) => {
            const on = opts.service === s.id;
            return (
              <div
                key={s.id}
                role="radio"
                aria-checked={on}
                tabIndex={0}
                onClick={() => pick(s.id)}
                onKeyDown={(e) => (e.key === 'Enter' || e.key === ' ') && (e.preventDefault(), pick(s.id))}
                className="cursor-pointer rounded-lg px-3 py-2"
                style={{ border: `1px solid ${on ? 'rgba(255,162,74,.55)' : 'rgba(255,255,255,.07)'}`, background: on ? 'rgba(255,162,74,.08)' : 'rgba(255,255,255,.02)' }}
              >
                <div style={{ font: fnt(500, 11.5), color: on ? '#ffc68a' : '#e8e2d8' }}>{s.name}</div>
                <div style={hint}>{s.note}</div>
              </div>
            );
          })}
        </div>
      </Step>

      {opts.service === 'ollama' && (
        <Step n={2} title="Ollama sur ce PC" >
          <div className="rounded-lg px-3 py-2.5 flex flex-col gap-2" style={{ border: '1px solid rgba(255,255,255,.07)' }} data-testid="llm-ollama">
            {!native ? (
              <span style={body}>La détection d’Ollama est disponible dans l’application Windows.</span>
            ) : checkingOllama && !ollama ? (
              <span style={body}>Recherche d’Ollama sur ce PC…</span>
            ) : ollama?.running && ollama.models.length > 0 ? (
              <>
                <span style={{ font: fnt(500, 11.5), color: '#7fe0a8' }}>
                  Ollama détecté · {ollama.models.length} modèle{ollama.models.length > 1 ? 's' : ''} installé{ollama.models.length > 1 ? 's' : ''}
                </span>
                <select className="text-input" aria-label="Modèle installé" value={opts.model} onChange={(e) => setLlm({ model: e.target.value })}>
                  {!ollama.models.includes(opts.model) && <option value={opts.model}>{opts.model || 'Choisissez un modèle'}</option>}
                  {ollama.models.map((m) => (
                    <option key={m} value={m}>
                      {m}
                    </option>
                  ))}
                </select>
                <span style={hint}>Aucune clé nécessaire : rien ne quitte votre PC.</span>
              </>
            ) : ollama?.running ? (
              <>
                <span style={{ font: fnt(500, 11.5), color: '#ffa24a' }}>Ollama détecté, mais aucun modèle n’est installé.</span>
                <span style={body}>Ouvrez un terminal (PowerShell) et tapez :</span>
                <code style={{ font: fnt(400, 11.5, 'mono'), color: '#e8e2d8' }}>{OLLAMA_PULL}</code>
              </>
            ) : (
              <>
                <span style={{ font: fnt(500, 11.5), color: '#ffa24a' }}>Ollama n’est pas détecté sur ce PC.</span>
                <span style={body}>
                  Installez Ollama (gratuit), puis dans un terminal (PowerShell) tapez <code style={{ font: fnt(400, 11, 'mono'), color: '#e8e2d8' }}>{OLLAMA_PULL}</code>. Ollama démarre tout seul ensuite.
                </span>
                <div className="flex gap-2">
                  <Pill ember onClick={() => llm.openKeyPage('ollama-download')}>
                    Télécharger Ollama ↗
                  </Pill>
                </div>
              </>
            )}
            {native && (
              <div className="flex">
                <Pill onClick={() => checkOllama()} disabled={checkingOllama}>
                  {checkingOllama ? 'Recherche…' : 'Vérifier à nouveau'}
                </Pill>
              </div>
            )}
          </div>
        </Step>
      )}

      {opts.service === 'lmstudio' && (
        <Step n={2} title="LM Studio sur ce PC">
          <span style={body}>Dans LM Studio, téléchargez un modèle, chargez-le, puis démarrez le serveur local (onglet « Developer »). Indiquez ici le nom du modèle chargé.</span>
          <input className="text-input" aria-label="Modèle" value={opts.model} placeholder="ex. qwen2.5-7b-instruct" maxLength={200} spellCheck={false} onChange={(e) => setLlm({ model: e.target.value })} />
        </Step>
      )}

      {!local && opts.service !== 'custom' && info && (
        <Step n={2} title="Créez votre clé">
          <div className="flex items-center gap-2">
            <Pill ember onClick={() => llm.openKeyPage(opts.service)} disabled={!native}>
              Obtenir ma clé {info.name} ↗
            </Pill>
          </div>
          <span style={hint}>La page du service s’ouvre dans votre navigateur : connectez-vous (ou créez un compte), créez une clé, puis copiez-la.</span>
        </Step>
      )}

      {!local && (
        <Step n={opts.service === 'custom' ? 2 : 3} title={opts.service === 'custom' ? 'Clé API (si le service en demande une)' : 'Collez-la ici'}>
          <div className="flex items-center gap-2">
            <input
              className="text-input min-w-0 flex-1"
              type="password"
              aria-label="Clé API"
              autoComplete="off"
              spellCheck={false}
              value={key}
              placeholder={status?.hasKey ? 'Clé enregistrée (non affichée)' : 'Collez votre clé'}
              maxLength={512}
              disabled={!native}
              onChange={(e) => setKey(e.target.value)}
              onKeyDown={(e) => e.key === 'Enter' && !busy && native && key.trim() && save()}
            />
            <Pill ember onClick={save} disabled={busy || !native || !key.trim()}>
              Enregistrer
            </Pill>
            <Pill onClick={clear} disabled={busy || !status?.hasKey}>
              Effacer
            </Pill>
          </div>
          <span style={hint} data-testid="llm-key-state">
            {!native
              ? 'Disponible dans l’application Windows.'
              : status?.hasKey
                ? `Clé enregistrée · ${status.keyStorage}. Elle n’est jamais réaffichée. Une seule clé est gardée : en changeant de service, collez la clé du nouveau service.`
                : 'Aucune clé enregistrée.'}
          </span>
          {keyError && (
            <span role="alert" style={{ font: fnt(400, 11, 'sans', 1.5), color: '#ff8a7a' }}>
              {keyError}
            </span>
          )}
        </Step>
      )}

      <div className="flex flex-col gap-1.5">
        <div className="flex items-center gap-2">
          <Pill ember onClick={() => runLlmTest(config)} disabled={!native || testing}>
            {testing ? 'Test en cours…' : 'Tester la connexion'}
          </Pill>
          <span style={hint}>Traduit une phrase d’exemple avec ces réglages.</span>
        </div>
        {shownTest && (
          <div className="rounded-lg px-3 py-2" style={{ border: `1px solid ${shownTest.ok ? 'rgba(127,224,168,.3)' : 'rgba(255,138,122,.3)'}` }} data-testid="llm-test">
            {shownTest.ok ? (
              <>
                <div style={{ font: fnt(500, 11.5), color: '#7fe0a8' }}>✓ Connexion réussie · {shownTest.ms} ms</div>
                <div style={body}>
                  « {shownTest.original} » → « {shownTest.translated} »
                </div>
                {!opts.enabled && <div style={hint}>Activez « Traduire avec un LLM en ligne » pour l’utiliser.</div>}
              </>
            ) : (
              <>
                <div style={{ font: fnt(500, 11.5), color: '#ff8a7a' }}>✗ {shownTest.error ?? 'échec'}</div>
                <div style={body}>{testAdvice(shownTest.error ?? '', opts.service)}</div>
              </>
            )}
          </div>
        )}
      </div>

      <div className="flex flex-col gap-2">
        <div role="button" aria-expanded={advanced} onClick={() => setAdvanced(!advanced)} className="cursor-pointer" style={{ font: fnt(500, 11), color: '#8f8a83' }}>
          {advanced ? '▾' : '▸'} Réglages avancés
        </div>
        {advanced && (
          <div className="flex flex-col gap-3" data-testid="llm-advanced">
            <div className="flex flex-col gap-1.5">
              <span style={label}>Format de l’API</span>
              <Seg options={PROVIDERS} value={opts.provider} onChange={(p) => setLlm({ provider: p, service: serviceOf({ provider: p, baseUrl: opts.baseUrl }) })} grid={2} item={{ padding: '6px 0', textAlign: 'center', font: fnt(500, 11.5) }} />
            </div>
            <label className="flex flex-col gap-1.5">
              <span style={label}>Adresse de base</span>
              <input
                className="text-input"
                aria-label="Adresse de base"
                value={opts.baseUrl}
                placeholder={LLM_DEFAULT_URL[opts.provider]}
                maxLength={300}
                spellCheck={false}
                onChange={(e) => setLlm({ baseUrl: e.target.value, service: serviceOf({ provider: opts.provider, baseUrl: e.target.value }) })}
              />
            </label>
            {opts.service !== 'lmstudio' && (
              <label className="flex flex-col gap-1.5">
                <span style={label}>Modèle</span>
                <input className="text-input" aria-label="Modèle" value={opts.model} placeholder={MODEL_EXAMPLES[opts.provider]} maxLength={200} spellCheck={false} onChange={(e) => setLlm({ model: e.target.value })} />
                <span style={hint}>Le modèle proposé est une suggestion : vous pouvez saisir n’importe quel modèle du service.</span>
              </label>
            )}
          </div>
        )}
      </div>

      <div className="flex flex-col gap-1 pt-1" data-testid="llm-state">
        <span style={{ font: fnt(500, 12), color: summary.color }}>{summary.value}</span>
        {status && opts.enabled && (
          <span style={body} data-testid="llm-stats">
            {llmStatsLine(status)}
          </span>
        )}
        {status?.lastError && opts.enabled && (
          <span style={{ font: fnt(400, 11, 'sans', 1.5), color: '#ff8a7a' }} data-testid="llm-last-error">
            Dernière erreur : {status.lastError}
          </span>
        )}
      </div>

      <div className="rounded-lg px-3 py-2.5" style={{ border: '1px solid rgba(255,162,74,.22)', background: 'rgba(255,162,74,.05)', font: fnt(400, 11, 'sans', 1.5), color: '#bdb6ab' }} data-testid="llm-privacy">
        <b style={{ color: '#ffc68a', fontWeight: 500 }}>Confidentialité.</b> Seul le texte des phrases est envoyé au service choisi : jamais l’audio, jamais les pseudos des joueurs. Le service traite ces textes selon ses propres conditions. Avec Ollama ou LM Studio sur ce PC, rien ne quitte votre réseau.
      </div>
    </div>
  );
}
