import { useState } from 'react';
import { LLM_DEFAULT_URL, type LlmProvider } from '../../shared/ipc';
import { Seg, ToggleRow, fnt } from '../../shared/ui/atoms';
import { useApp } from '../../app/state/store';
import { clearLlmKey, llmStatsLine, llmSummary, MODEL_EXAMPLES, providerChange, saveLlmKey, useLlm } from './llm';

const PROVIDERS: [LlmProvider, string][] = [
  ['openai', 'Compatible OpenAI'],
  ['anthropic', 'Anthropic'],
];

const label = { font: fnt(500, 11), color: '#bdb6ab' };
const hint = { font: fnt(400, 10.5, 'sans', 1.5), color: '#6e6961' };

/** Audio & IA › online translation by an LLM, with the user's own provider and key. */
export function LlmCard() {
  const opts = useApp((s) => s.llm);
  const setLlm = useApp((s) => s.setLlm);
  const { native, status } = useLlm();
  const [key, setKey] = useState('');
  const [busy, setBusy] = useState(false);
  const [keyError, setKeyError] = useState<string | null>(null);
  const summary = !native && opts.enabled ? { value: 'État disponible dans l’application Windows', color: '#8f8a83' } : llmSummary(status, opts.enabled);

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
        <span style={{ font: fnt(500, 9.5, 'mono'), letterSpacing: '.12em', color: '#6e6961' }}>VOTRE FOURNISSEUR · VOTRE CLÉ</span>
      </div>
      <div style={{ font: fnt(400, 11, 'sans', 1.5), color: '#8f8a83' }}>
        Facultatif : les phrases peuvent être traduites par le LLM de votre choix, avec votre propre clé API. En cas de problème (réseau, quota, clé refusée, délai dépassé, réponse incohérente), Opus-MT traduit sur ce PC. Les lignes en direct restent toujours locales.
      </div>
      <ToggleRow title="Traduire avec un LLM en ligne" sub="Désactivé par défaut" on={opts.enabled} onClick={() => setLlm({ enabled: !opts.enabled })} />

      <div className="flex flex-col gap-1.5">
        <span style={label}>Fournisseur</span>
        <Seg options={PROVIDERS} value={opts.provider} onChange={(p) => setLlm(providerChange(p, opts))} grid={2} item={{ padding: '6px 0', textAlign: 'center', font: fnt(500, 11.5) }} />
      </div>
      <label className="flex flex-col gap-1.5">
        <span style={label}>Adresse de base</span>
        <input className="text-input" aria-label="Adresse de base" value={opts.baseUrl} placeholder={LLM_DEFAULT_URL[opts.provider]} maxLength={300} spellCheck={false} onChange={(e) => setLlm({ baseUrl: e.target.value })} />
        <span style={hint}>
          {opts.provider === 'openai'
            ? 'OpenAI, OpenRouter (https://openrouter.ai/api/v1), Mistral, Groq, ou un serveur local : Ollama (http://localhost:11434/v1), LM Studio (http://localhost:1234/v1).'
            : 'API Anthropic (https://api.anthropic.com/v1).'}
        </span>
      </label>
      <label className="flex flex-col gap-1.5">
        <span style={label}>Modèle</span>
        <input className="text-input" aria-label="Modèle" value={opts.model} placeholder={MODEL_EXAMPLES[opts.provider]} maxLength={200} spellCheck={false} onChange={(e) => setLlm({ model: e.target.value })} />
      </label>

      <div className="flex flex-col gap-1.5">
        <span style={label}>Clé API</span>
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
          />
          <div className="pill pill-ember flex-none px-3.5 py-[7px]" role="button" aria-disabled={busy || !native || !key.trim()} onClick={() => !busy && native && key.trim() && save()} style={{ font: fnt(500, 11.5) }}>
            Enregistrer
          </div>
          <div className="pill pill-ghost flex-none px-3.5 py-[7px]" role="button" aria-disabled={busy || !status?.hasKey} onClick={() => !busy && status?.hasKey && clear()} style={{ font: fnt(500, 11.5) }}>
            Effacer
          </div>
        </div>
        <span style={hint} data-testid="llm-key-state">
          {!native
            ? 'Disponible dans l’application Windows.'
            : status?.hasKey
              ? `Clé enregistrée · ${status.keyStorage}. Elle n’est jamais réaffichée.`
              : 'Aucune clé enregistrée. Inutile pour un serveur local (Ollama, LM Studio).'}
        </span>
        {keyError && (
          <span role="alert" style={{ font: fnt(400, 11, 'sans', 1.5), color: '#ff8a7a' }}>
            {keyError}
          </span>
        )}
      </div>

      <div className="flex flex-col gap-1 pt-1" data-testid="llm-state">
        <span style={{ font: fnt(500, 12), color: summary.color }}>{summary.value}</span>
        {status && opts.enabled && (
          <span style={{ font: fnt(400, 11, 'sans', 1.5), color: '#8f8a83' }} data-testid="llm-stats">
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
        <b style={{ color: '#ffc68a', fontWeight: 500 }}>Confidentialité.</b> Seul le texte des phrases est envoyé au fournisseur choisi : jamais l’audio, jamais les pseudos des joueurs. Le fournisseur traite ces textes selon ses propres conditions. Avec un serveur local (Ollama, LM Studio), rien ne quitte votre réseau.
      </div>
    </div>
  );
}
