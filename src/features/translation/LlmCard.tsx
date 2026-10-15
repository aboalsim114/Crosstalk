import { useEffect, useState, type ReactNode } from 'react';
import { llm, LLM_DEFAULT_URL, LLM_SERVICES, llmService, serviceOf, type LlmProvider, type LlmService, type LocalServer } from '../../shared/ipc';
import { Seg, fnt } from '../../shared/ui/atoms';
import { useApp } from '../../app/state/store';
import { NATIVE_NAME, PLAYER_LANGS, translatedSources, type PlayerLang } from '../languages/langs';
import { clearLlmKey, detectLocal, isLocalChoice, llmConfigFrom, llmStatsLine, llmSummary, loadModels, localServer, MODEL_EXAMPLES, runLlmTest, runTranslateTest, saveLlmKey, serviceChange, testAdvice, testKey, useLlm } from './llm';

const PROVIDERS: [LlmProvider, string][] = [
  ['openai', 'Compatible OpenAI'],
  ['anthropic', 'Anthropic'],
];

/** Suggested first model for Ollama: small, multilingual, runs on most gaming PCs. Typed by the user, never run by Crosstalk. */
export const OLLAMA_PULL = 'ollama pull qwen2.5:7b';

/** The corpus sentence of the migration, ready to try. */
export const TRY_SENTENCE = 'Mon équipe ne sait pas jouer.';

const CLOUD_SERVICES = LLM_SERVICES.filter((s) => !s.local);
const LOCAL_NAMES: Record<LocalServer['kind'], string> = { ollama: 'Ollama', lmstudio: 'LM Studio' };

const label = { font: fnt(500, 11), color: '#bdb6ab' };
const hint = { font: fnt(400, 10.5, 'sans', 1.5), color: '#6e6961' };
const body = { font: fnt(400, 11, 'sans', 1.5), color: '#8f8a83' };
const btn = { font: fnt(500, 11.5) };
const errorText = { font: fnt(400, 11, 'sans', 1.5), color: '#ff8a7a' };

function Section({ title, children }: { title: string; children: ReactNode }) {
  return (
    <div className="flex flex-col gap-1.5">
      <span style={label}>{title}</span>
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

function Choice({ on, title, sub, onPick, testId, children }: { on: boolean; title: string; sub: string; onPick: () => void; testId?: string; children?: ReactNode }) {
  return (
    <div
      role="radio"
      aria-checked={on}
      tabIndex={0}
      data-testid={testId}
      onClick={onPick}
      onKeyDown={(e) => (e.key === 'Enter' || e.key === ' ') && (e.preventDefault(), onPick())}
      className="cursor-pointer rounded-lg px-3 py-2"
      style={{ border: `1px solid ${on ? 'rgba(255,162,74,.55)' : 'rgba(255,255,255,.07)'}`, background: on ? 'rgba(255,162,74,.08)' : 'rgba(255,255,255,.02)' }}
    >
      <div style={{ font: fnt(500, 11.5), color: on ? '#ffc68a' : '#e8e2d8' }}>{title}</div>
      <div style={hint}>{sub}</div>
      {children}
    </div>
  );
}

function Info({ k, v, color }: { k: string; v: string; color?: string }) {
  return (
    <>
      <span style={hint}>{k}</span>
      <span className="min-w-0 truncate" title={v} style={{ font: fnt(400, 11), color: color ?? '#d8d2c8' }}>
        {v}
      </span>
    </>
  );
}

/** What a local server card says: running with models, running empty, or absent. */
export function localState(server: LocalServer | null, detected: boolean): { text: string; color: string } {
  if (!detected) return { text: 'Recherche…', color: '#8f8a83' };
  if (!server?.running) return { text: 'Non détecté sur ce PC', color: '#8f8a83' };
  const n = server.models.length;
  if (!n) return { text: 'Démarré, aucun modèle disponible', color: '#ffa24a' };
  return { text: `Détecté · ${n} modèle${n > 1 ? 's' : ''}`, color: '#7fe0a8' };
}

/** Audio & IA › TRADUCTION IA: the only translation engine, a cloud provider or a LLM on this PC. */
export function LlmCard() {
  const opts = useApp((s) => s.llm);
  const setLlm = useApp((s) => s.setLlm);
  const target = useApp((s) => s.target);
  const src = useApp((s) => s.src);
  const { native, status, test, testedWith, testing, local, detecting, models, modelsFor, loadingModels } = useLlm();
  const isLocal = isLocalChoice(opts);
  const [mode, setMode] = useState<'cloud' | 'local'>(isLocal ? 'local' : 'cloud');
  const [key, setKey] = useState('');
  const [busy, setBusy] = useState(false);
  const [keyError, setKeyError] = useState<string | null>(null);
  const [connectError, setConnectError] = useState<string | null>(null);
  const [tryText, setTryText] = useState(TRY_SENTENCE);
  const [trySource, setTrySource] = useState<PlayerLang>('fr');
  const [tryTarget, setTryTarget] = useState<PlayerLang>(target === 'fr' ? 'ru' : target);
  const info = llmService(opts.service);
  const config = llmConfigFrom({ llm: opts });
  // A result is shown only for the settings it was run with.
  const shownTest = test && testedWith === testKey(config) ? test : null;
  const server = localServer(local, opts.service);
  const listed = modelsFor === `${opts.provider}|${opts.baseUrl.trim()}` ? models : null;
  const modelChoices = mode === 'local' && server ? server.models : (listed?.models ?? []);
  const summary = !native && opts.enabled ? { value: 'État disponible dans l’application Windows', color: '#8f8a83' } : llmSummary(status, opts.enabled);
  const serviceName = info?.name ?? (opts.baseUrl.trim() || 'adresse personnalisée');
  const langs = translatedSources(target, src);

  useEffect(() => {
    if (mode === 'local' && native && !local && !detecting) detectLocal();
  }, [mode, native, local, detecting]);

  const switchMode = (next: 'cloud' | 'local') => {
    setMode(next);
    setConnectError(null);
    if (next === (isLocal ? 'local' : 'cloud')) return;
    // A provider change disconnects: the user connects again once the new one is ready.
    if (next === 'cloud') setLlm({ ...serviceChange('openai', opts), enabled: false });
    else setLlm({ ...serviceChange('ollama', opts, localServer(local, 'ollama')?.models), enabled: false });
  };
  const pick = (service: LlmService) => {
    setConnectError(null);
    if (service === opts.service) return;
    const models = localServer(local, service)?.models;
    setLlm({ ...serviceChange(service, opts, models), enabled: false, ...(service === 'custom' && mode === 'local' ? { provider: 'openai' as const, baseUrl: 'http://localhost:8080/v1', model: '' } : {}) });
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
  const connect = () => {
    if (!opts.model.trim()) return setConnectError('Choisissez un modèle.');
    if (mode === 'cloud' && !opts.cloudConsent) return setConnectError(`Acceptez d’abord l’envoi du texte des phrases à ${serviceName}.`);
    if (mode === 'cloud' && native && opts.service !== 'custom' && !status?.hasKey) return setConnectError('Enregistrez d’abord votre clé API.');
    setConnectError(null);
    setLlm({ enabled: true });
  };

  return (
    <div className="glass flex flex-col gap-3 p-5" data-testid="llm-card">
      <div className="flex items-center justify-between">
        <span className="lbl">TRADUCTION IA</span>
        <span style={{ font: fnt(500, 9.5, 'mono'), letterSpacing: '.12em', color: opts.enabled ? '#7fe0a8' : '#6e6961' }} data-testid="llm-badge">
          {opts.enabled ? (isLocal ? 'LOCAL · CONNECTÉ' : 'CLOUD · CONNECTÉ') : 'NON CONNECTÉ'}
        </span>
      </div>
      <div style={body}>
        Un LLM traduit les voix, le chat et vos réponses. Choisissez un fournisseur en ligne avec votre propre clé, ou un LLM installé sur ce PC (Ollama, LM Studio). Tant qu’aucun moteur n’est connecté, les phrases restent dans leur langue d’origine.
      </div>

      <Seg
        options={[
          ['cloud', 'Cloud — Fournisseur API'],
          ['local', 'Local — LLM installé sur ce PC'],
        ]}
        value={mode}
        onChange={switchMode}
        grid={2}
        item={{ padding: '7px 0', textAlign: 'center', font: fnt(500, 11.5) }}
      />

      {mode === 'cloud' ? (
        <>
          <Section title="Fournisseur">
            <div className="grid grid-cols-2 gap-1.5" role="radiogroup" aria-label="Fournisseur">
              {CLOUD_SERVICES.map((s) => (
                <Choice key={s.id} on={opts.service === s.id} title={s.name} sub={s.note} onPick={() => pick(s.id)} />
              ))}
              <Choice on={opts.service === 'custom'} title="Autre compatible OpenAI" sub="Adresse et modèle à saisir" onPick={() => pick('custom')} />
            </div>
          </Section>

          {opts.service === 'custom' && <AddressFields />}

          <Section title={opts.service === 'custom' ? 'Clé API (si le service en demande une)' : 'Clé API'}>
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
            <div className="flex flex-wrap items-center gap-2">
              {info && (
                <Pill onClick={() => llm.openKeyPage(opts.service)} disabled={!native}>
                  Obtenir ma clé {info.name} ↗
                </Pill>
              )}
              <span style={hint} data-testid="llm-key-state">
                {!native
                  ? 'Disponible dans l’application Windows.'
                  : status?.hasKey
                    ? `Clé enregistrée · ${status.keyStorage}. Jamais réaffichée. Une seule clé est gardée : en changeant de fournisseur, collez la clé du nouveau.`
                    : 'Aucune clé enregistrée.'}
              </span>
            </div>
            {keyError && (
              <span role="alert" style={errorText}>
                {keyError}
              </span>
            )}
          </Section>
        </>
      ) : (
        <>
          <Section title="LLM sur ce PC">
            <div className="grid grid-cols-2 gap-1.5" role="radiogroup" aria-label="Serveur local">
              {(['ollama', 'lmstudio'] as const).map((id) => {
                const st = localState(localServer(local, id), !!local || !native);
                return (
                  <Choice key={id} on={opts.service === id} title={LOCAL_NAMES[id]} sub={native ? '' : 'Détection dans l’application Windows'} onPick={() => pick(id)} testId={`llm-local-${id}`}>
                    {native && <div style={{ font: fnt(500, 10.5), color: st.color }}>{st.text}</div>}
                  </Choice>
                );
              })}
            </div>
            <Choice on={opts.service === 'custom'} title="Adresse manuelle (avancé)" sub="Un autre serveur compatible OpenAI sur ce PC ou le réseau local" onPick={() => pick('custom')} />
            {native && (
              <div className="flex items-center gap-2">
                <Pill onClick={() => detectLocal()} disabled={detecting}>
                  {detecting ? 'Recherche…' : 'Actualiser les modèles locaux'}
                </Pill>
                <span style={hint}>Aucune clé nécessaire : les phrases restent sur ce PC.</span>
              </div>
            )}
          </Section>

          {opts.service === 'custom' && <AddressFields />}

          {native && local && (opts.service === 'ollama' || opts.service === 'lmstudio') && !server?.running && (
            <div className="flex flex-col gap-2 rounded-lg px-3 py-2.5" style={{ border: '1px solid rgba(255,255,255,.07)' }} data-testid="llm-local-help">
              {opts.service === 'ollama' ? (
                <span style={body}>
                  Ollama n’est pas détecté. Installez-le (gratuit), puis dans PowerShell tapez <code style={{ font: fnt(400, 11, 'mono'), color: '#e8e2d8' }}>{OLLAMA_PULL}</code>. Crosstalk ne télécharge aucun modèle.
                </span>
              ) : (
                <span style={body}>LM Studio n’est pas détecté. Dans LM Studio, chargez un modèle puis démarrez le serveur local (onglet « Developer »). Crosstalk ne télécharge aucun modèle.</span>
              )}
              <div className="flex">
                <Pill onClick={() => llm.openKeyPage(opts.service === 'ollama' ? 'ollama-download' : 'lmstudio-download')}>Télécharger {LOCAL_NAMES[opts.service]} ↗</Pill>
              </div>
            </div>
          )}
          {native && server?.running && server.models.length === 0 && (
            <span style={{ ...body, color: '#ffa24a' }} data-testid="llm-local-empty">
              {opts.service === 'ollama' ? `Aucun modèle installé dans Ollama. Dans PowerShell, tapez ${OLLAMA_PULL}, puis actualisez.` : 'Aucun modèle chargé dans LM Studio : chargez-en un, puis actualisez.'}
            </span>
          )}
        </>
      )}

      <Section title="Modèle">
        <div className="flex items-center gap-2">
          {modelChoices.length > 0 && (
            <select className="text-input min-w-0 flex-1" aria-label="Modèle disponible" value={modelChoices.includes(opts.model) ? opts.model : ''} onChange={(e) => e.target.value && setLlm({ model: e.target.value })}>
              {!modelChoices.includes(opts.model) && <option value="">{opts.model ? `${opts.model} (saisi)` : 'Choisissez un modèle'}</option>}
              {modelChoices.map((m) => (
                <option key={m} value={m}>
                  {m}
                </option>
              ))}
            </select>
          )}
          <input className="text-input min-w-0 flex-1" aria-label="Modèle" value={opts.model} placeholder={MODEL_EXAMPLES[opts.provider]} maxLength={200} spellCheck={false} onChange={(e) => setLlm({ model: e.target.value })} />
          {mode === 'cloud' && (
            <Pill onClick={() => loadModels(config)} disabled={!native || loadingModels}>
              {loadingModels ? 'Chargement…' : 'Lister les modèles'}
            </Pill>
          )}
        </div>
        {listed?.error && mode === 'cloud' && <span style={errorText}>Liste indisponible : {listed.error}</span>}
        <span style={hint}>{mode === 'cloud' ? 'Le modèle proposé est une suggestion : choisissez-en un dans la liste du fournisseur ou saisissez son nom.' : 'Les modèles viennent du serveur local : aucun n’est téléchargé par Crosstalk.'}</span>
      </Section>

      {mode === 'cloud' && (
        <div
          role="checkbox"
          aria-checked={opts.cloudConsent}
          tabIndex={0}
          data-testid="llm-consent"
          onClick={() => setLlm({ cloudConsent: !opts.cloudConsent, ...(opts.cloudConsent ? { enabled: false } : {}) })}
          onKeyDown={(e) => (e.key === 'Enter' || e.key === ' ') && (e.preventDefault(), setLlm({ cloudConsent: !opts.cloudConsent, ...(opts.cloudConsent ? { enabled: false } : {}) }))}
          className="flex cursor-pointer items-start gap-2.5 rounded-lg px-3 py-2.5"
          style={{ border: '1px solid rgba(255,162,74,.22)', background: 'rgba(255,162,74,.05)' }}
        >
          <span className="mt-[2px] flex h-[14px] w-[14px] flex-none items-center justify-center rounded-[3px]" style={{ border: '1px solid rgba(255,162,74,.6)', background: opts.cloudConsent ? '#ffa24a' : 'transparent', color: '#120f0c', font: fnt(700, 10) }}>
            {opts.cloudConsent ? '✓' : ''}
          </span>
          <span style={{ font: fnt(400, 11, 'sans', 1.5), color: '#bdb6ab' }}>
            J’accepte que le <b style={{ color: '#ffc68a', fontWeight: 500 }}>texte des phrases</b> (voix transcrites, chat, mes réponses) soit envoyé à {serviceName}, qui le traite selon ses propres conditions. Jamais l’audio, jamais les pseudos des joueurs.
          </span>
        </div>
      )}

      <div className="flex flex-wrap items-center gap-2">
        {opts.enabled ? (
          <Pill onClick={() => setLlm({ enabled: false })}>Déconnecter</Pill>
        ) : (
          <Pill ember onClick={connect}>
            Connecter
          </Pill>
        )}
        <Pill onClick={() => runLlmTest(config)} disabled={!native || testing}>
          {testing ? 'Test en cours…' : 'Tester la connexion'}
        </Pill>
      </div>
      {connectError && (
        <span role="alert" style={errorText} data-testid="llm-connect-error">
          {connectError}
        </span>
      )}

      <Section title="Tester une traduction">
        <div className="flex items-center gap-2">
          <select className="text-input flex-none" aria-label="Langue de la phrase" value={trySource} onChange={(e) => setTrySource(e.target.value as PlayerLang)}>
            {PLAYER_LANGS.map((l) => (
              <option key={l} value={l}>
                {NATIVE_NAME[l]}
              </option>
            ))}
          </select>
          <span style={hint}>→</span>
          <select className="text-input flex-none" aria-label="Traduire en" value={tryTarget} onChange={(e) => setTryTarget(e.target.value as PlayerLang)}>
            {PLAYER_LANGS.map((l) => (
              <option key={l} value={l}>
                {NATIVE_NAME[l]}
              </option>
            ))}
          </select>
        </div>
        <div className="flex items-center gap-2">
          <input className="text-input min-w-0 flex-1" aria-label="Phrase à essayer" value={tryText} maxLength={500} onChange={(e) => setTryText(e.target.value)} />
          <Pill ember onClick={() => runTranslateTest(config, tryText, trySource, tryTarget)} disabled={!native || testing || !tryText.trim() || trySource === tryTarget}>
            Traduire
          </Pill>
        </div>
      </Section>

      {shownTest && (
        <div className="rounded-lg px-3 py-2" style={{ border: `1px solid ${shownTest.ok ? 'rgba(127,224,168,.3)' : 'rgba(255,138,122,.3)'}` }} data-testid="llm-test">
          {shownTest.ok ? (
            <>
              <div style={{ font: fnt(500, 11.5), color: '#7fe0a8' }}>✓ Réponse en {shownTest.ms} ms</div>
              <div style={body}>
                « {shownTest.original} » → « {shownTest.translated} »
              </div>
              {!opts.enabled && <div style={hint}>Cliquez sur « Connecter » pour traduire les parties avec ce moteur.</div>}
            </>
          ) : (
            <>
              <div style={{ font: fnt(500, 11.5), color: '#ff8a7a' }}>✗ {shownTest.error ?? 'échec'}</div>
              <div style={body}>{testAdvice(shownTest.error ?? '', opts.service)}</div>
            </>
          )}
        </div>
      )}

      <div className="flex flex-col gap-2 rounded-lg px-3 py-2.5" style={{ border: '1px solid rgba(255,255,255,.06)', background: 'rgba(255,255,255,.015)' }} data-testid="llm-state">
        <span style={{ font: fnt(500, 12), color: summary.color }}>{summary.value}</span>
        <div className="grid gap-x-3 gap-y-1" style={{ gridTemplateColumns: 'auto minmax(0,1fr)' }}>
          <Info k="Fournisseur" v={serviceName} />
          <Info k="Modèle" v={opts.model.trim() || '—'} />
          <Info k="Type" v={isLocal ? 'Local (rien ne quitte ce PC ou le réseau local)' : 'Cloud (texte envoyé au fournisseur)'} />
          {isLocal && <Info k="Serveur local" v={status?.backend ?? (info?.local ? LOCAL_NAMES[opts.service as LocalServer['kind']] : 'adresse manuelle')} />}
          <Info k="Langues" v={`${langs.map((l) => l.toUpperCase()).join(', ') || '—'} → ${target.toUpperCase()}`} />
          {status && <Info k="Activité" v={llmStatsLine(status)} />}
          {status?.lastError && <Info k="Dernière erreur" v={status.lastError} color="#ff8a7a" />}
        </div>
      </div>
    </div>
  );
}

/** Address and API format of a server the list does not know. */
function AddressFields() {
  const opts = useApp((s) => s.llm);
  const setLlm = useApp((s) => s.setLlm);
  return (
    <div className="flex flex-col gap-3" data-testid="llm-advanced">
      <div className="flex flex-col gap-1.5">
        <span style={label}>Format de l’API</span>
        <Seg options={PROVIDERS} value={opts.provider} onChange={(p) => setLlm({ provider: p, enabled: false })} grid={2} item={{ padding: '6px 0', textAlign: 'center', font: fnt(500, 11.5) }} />
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
          onChange={(e) => {
            const service = serviceOf({ provider: opts.provider, baseUrl: e.target.value });
            setLlm({ baseUrl: e.target.value, service: service === 'custom' ? 'custom' : opts.service, enabled: false });
          }}
        />
        <span style={hint}>https:// obligatoire ; http:// seulement pour ce PC ou le réseau local.</span>
      </label>
    </div>
  );
}
