import { useEffect, useRef, useState } from 'react';
import { translation, type TranslationPlan, type TranslationResult, type TranslationSource, type TranslationStatus } from '../backend/translation';
import { translationGlossary, useTranslation } from '../state/translation';
import { useApp } from '../state/store';
import { fnt } from './atoms';

/** Standalone manual test for #10. Speech/session publication belongs to #11. */
export function TranslationCard() {
  const status = useTranslation((s) => s.status);
  const [key, setKey] = useState('');
  const [text, setText] = useState('Two players on B, wait for my flash.');
  const [source, setSource] = useState<TranslationSource>('en');
  const [result, setResult] = useState<TranslationResult | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [testing, setTesting] = useState(false);
  const alive = useRef(false);
  const revision = useRef(0);
  const testId = useRef(0);
  const mutations = useRef<Promise<unknown>>(Promise.resolve());
  const plan = status?.plan ?? 'free';

  useEffect(() => {
    alive.current = true;
    const current = revision.current;
    translation.status().then((next) => {
      if (alive.current && current === revision.current) useTranslation.setState({ status: next });
    }).catch(() => {
      if (alive.current && current === revision.current) setError('État de traduction indisponible.');
    });
    return () => { alive.current = false; revision.current++; testId.current++; };
  }, []);

  const invalidate = () => {
    revision.current++;
    testId.current++;
    setResult(null);
    setTesting(false);
    setError(null);
  };
  function serialize<T>(run: () => Promise<T>): Promise<T> {
    const pending = mutations.current.then(run, run);
    mutations.current = pending.catch(() => undefined);
    return pending;
  }
  async function change(run: () => Promise<TranslationStatus>) {
    invalidate();
    const current = revision.current;
    setSaving(true);
    try {
      const next = await serialize(run);
      if (alive.current && current === revision.current) useTranslation.setState({ status: next });
    } catch {
      // Never echo an exception: it may contain a provider response or a key.
      if (alive.current && current === revision.current) setError('Modification impossible. Vérifie la configuration de DeepL.');
    } finally {
      if (alive.current && current === revision.current) setSaving(false);
    }
  }
  const configure = (enabled: boolean, nextPlan: TranslationPlan) => translation.configure({
    enabled, plan: nextPlan, glossary: translationGlossary(useApp.getState().gloss),
  });
  async function test() {
    const current = revision.current;
    const id = ++testId.current;
    setTesting(true);
    setError(null);
    setResult(null);
    try {
      const next = await serialize(() => configure(status?.enabled ?? false, plan));
      if (!alive.current || current !== revision.current || id !== testId.current) return;
      useTranslation.setState({ status: next });
      const value = await translation.test(text, source);
      if (alive.current && current === revision.current && id === testId.current) setResult(value);
    } catch {
      if (alive.current && current === revision.current && id === testId.current) setError('Test indisponible. Le texte original est conservé.');
    } finally {
      if (alive.current && id === testId.current) setTesting(false);
    }
  }

  const supported = status?.supported ?? false;
  const canTest = supported && text.trim().length > 0 && (source === 'fr' || !!(status?.enabled && status.keyConfigured));
  const labelStyle = { font: fnt(400, 11.5), color: '#8f8a83' };
  return (
    <section className="glass flex flex-col gap-3 p-5" aria-label="Traduction DeepL">
      <div className="lbl">MOTEUR DE TRADUCTION · DEEPL</div>
      <p style={labelStyle}>Test de texte en français. La traduction automatique des voix arrive à l’étape suivante.</p>
      <label className="flex items-center justify-between gap-3" style={labelStyle}>
        Compte DeepL API
        <select className="text-input" aria-label="Compte DeepL API" value={plan} disabled={!supported || saving} onChange={(e) => {
          const nextPlan = e.target.value as TranslationPlan;
          setKey('');
          void change(() => configure(false, nextPlan));
        }}>
          <option value="free">API Free</option><option value="pro">API Pro</option>
        </select>
      </label>
      <form className="flex flex-col gap-2" onSubmit={(e) => {
        e.preventDefault();
        const secret = key;
        setKey('');
        void change(() => translation.saveKey(plan, secret));
      }}>
        <label style={labelStyle} htmlFor="deepl-api-key">Clé API DeepL</label>
        <input id="deepl-api-key" className="text-input" type="password" autoComplete="off" maxLength={4096} value={key} disabled={!supported || saving} onChange={(e) => setKey(e.target.value)} />
        <div className="flex gap-2">
          <button className="pill pill-ember px-3 py-2" type="submit" disabled={!supported || saving || !key.trim()}>Enregistrer la clé</button>
          <button className="pill px-3 py-2" type="button" disabled={!status?.keyConfigured || saving} onClick={() => { setKey(''); void change(() => translation.deleteKey(plan)); }}>Supprimer la clé</button>
        </div>
      </form>
      <p style={labelStyle}>{status?.keyConfigured ? 'Clé enregistrée dans le Gestionnaire d’identification Windows.' : 'Aucune clé configurée pour ce compte.'} La clé n’est jamais affichée ni conservée dans un fichier de réglages.</p>
      <label className="flex items-center gap-3" style={labelStyle}>
        <input type="checkbox" aria-label="Activer les tests DeepL" checked={status?.enabled ?? false} disabled={!supported || !status?.keyConfigured || saving} onChange={(e) => { const enabled = e.target.checked; void change(() => configure(enabled, plan)); }} />
        Activer les tests DeepL
      </label>
      <p style={labelStyle}>Après activation, le texte saisi ci-dessous est envoyé à DeepL. L’activation est désactivée à chaque démarrage de l’application.</p>
      <label style={labelStyle}>Langue du texte
        <select className="text-input ml-2" aria-label="Langue du texte" value={source} disabled={testing} onChange={(e) => { setSource(e.target.value as TranslationSource); setResult(null); }}>
          <option value="en">Anglais</option><option value="ru">Russe</option><option value="fr">Français</option>
        </select>
      </label>
      <label htmlFor="translation-test-text" style={labelStyle}>Phrase à tester</label>
      <textarea id="translation-test-text" className="text-input" rows={3} maxLength={2048} value={text} disabled={testing} onChange={(e) => { setText(e.target.value); setResult(null); }} />
      <button className="pill pill-ember px-3 py-2" type="button" disabled={!canTest || testing || saving} onClick={() => { void test(); }}>{testing ? 'Traduction…' : source === 'fr' ? 'Afficher l’original français' : 'Tester la traduction'}</button>
      {!supported && <p role="status" style={labelStyle}>Configuration disponible dans l’application Windows.</p>}
      {error && <p role="alert" className="warn-box">{error}</p>}
      {result && <div role="status" className="flex flex-col gap-2" style={labelStyle}>
        <span className="lbl">{result.status === 'translated' ? 'TRADUCTION FRANÇAISE' : 'ORIGINAL'}</span>
        <p style={{ font: fnt(400, 18, 'serif'), color: '#ece6dc' }}>{result.translated ?? result.original}</p>
        {result.reason && <p>{result.reason}</p>}
        <span>{result.latencyMs} ms{result.cached ? ' · cache' : ''}</span>
      </div>}
      <p style={labelStyle}>Azure et le moteur local ne sont pas disponibles dans cette version.</p>
    </section>
  );
}
