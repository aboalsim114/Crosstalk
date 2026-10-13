import { useEffect, useRef, useState } from 'react';
import { dictation, isTauri, reply, type ReplyPhrase, type ReplyTarget } from '../backend';
import { fnt } from '../components/atoms';

export const TARGET_NAME: Record<ReplyTarget, string> = { en: 'Anglais', ru: 'Russe' };

/** Copies to the clipboard (Clipboard API, or the old way if it is refused). */
export async function copyText(text: string): Promise<boolean> {
  try {
    await navigator.clipboard.writeText(text);
    return true;
  } catch {
    const ta = document.createElement('textarea');
    ta.value = text;
    document.body.appendChild(ta);
    ta.select();
    const ok = document.execCommand('copy');
    ta.remove();
    return ok;
  }
}

/** Teammates' language from the recent lines (voice and chat). */
export function teammatesLanguage(langs: string[]): ReplyTarget {
  const ru = langs.filter((l) => l === 'ru').length;
  const en = langs.filter((l) => l === 'en').length;
  return ru > en ? 'ru' : 'en';
}

export const PASTE_HINT = 'Copié ✓ — dans CS2 : Y (tous) ou U (équipe), Ctrl+V, Entrée';

interface Props {
  target: ReplyTarget;
  onTarget: (t: ReplyTarget) => void;
  /** Called once the text is in the clipboard. */
  onCopied?: () => void;
  autoFocusKey?: number;
}

/** French in, the teammates' language out, copied to the clipboard. */
export function ReplyBox({ target, onTarget, onCopied, autoFocusKey }: Props) {
  const [text, setText] = useState('');
  const [phrases, setPhrases] = useState<ReplyPhrase[]>([]);
  const [busy, setBusy] = useState(false);
  const [out, setOut] = useState<{ text: string; note: string; error?: boolean } | null>(null);
  const input = useRef<HTMLInputElement>(null);

  useEffect(() => {
    reply.phrases().then(setPhrases);
  }, []);
  useEffect(() => {
    input.current?.focus();
  }, [autoFocusKey]);

  const done = async (translated: string, note: string) => {
    const ok = await copyText(translated);
    setOut({ text: translated, note: ok ? `${note} · ${PASTE_HINT}` : 'Copie impossible : sélectionnez le texte ci-dessus.' });
    if (ok) onCopied?.();
  };
  const send = async () => {
    const t = text.trim();
    if (!t || busy) return;
    setBusy(true);
    setOut({ text: '…', note: 'Traduction locale en cours (le premier message charge le modèle, quelques secondes)' });
    try {
      const r = await reply.translate(t, target);
      await done(r.text, r.via === 'phrasebook' ? 'Phrase CS vérifiée' : `Traduit sur ce PC en ${(r.ms / 1000).toFixed(1).replace('.', ',')} s`);
      setText('');
    } catch (e) {
      setOut({ text: '', note: String(e), error: true });
    } finally {
      setBusy(false);
    }
  };
  const quick = async (p: ReplyPhrase) => {
    const t = await reply.phraseText(p.id, target);
    if (t) await done(t, 'Phrase CS vérifiée');
  };

  return (
    <div className="flex flex-col gap-3">
      <div className="flex items-center gap-2">
        <span className="lbl">RÉPONDRE EN</span>
        {(['en', 'ru'] as const).map((t) => (
          <div
            key={t}
            role="button"
            aria-pressed={target === t}
            onClick={() => onTarget(t)}
            className={`pill px-3 py-1 ${target === t ? 'pill-ember' : ''}`}
            style={{ font: fnt(500, 11.5) }}
          >
            {TARGET_NAME[t]}
          </div>
        ))}
      </div>
      <div className="flex gap-2">
        <input
          ref={input}
          className="text-input"
          aria-label="Message en français"
          placeholder="Écrivez en français, puis Entrée…"
          value={text}
          maxLength={300}
          disabled={busy}
          onChange={(e) => setText(e.target.value)}
          onKeyDown={(e) => e.key === 'Enter' && send()}
          style={{ flex: 1, minWidth: 0 }}
        />
        {isTauri && (
          <div className="pill flex-none px-3 py-2" role="button" title="Dicter en français (Ctrl+Alt+F6)" onClick={() => void dictation.toggle([], target)} style={{ font: fnt(500, 12) }}>
            🎙 Dicter
          </div>
        )}
      </div>
      {phrases.length > 0 && (
        <div className="flex flex-wrap gap-1.5">
          {phrases.map((p) => (
            <div key={p.id} role="button" className="pill px-2.5 py-1" onClick={() => quick(p)} style={{ font: fnt(400, 11) }}>
              {p.fr}
            </div>
          ))}
        </div>
      )}
      {out && (
        <div className="flex flex-col gap-1 rounded-xl px-3 py-2.5" style={{ background: 'rgba(0,0,0,.35)', border: '1px solid rgba(255,255,255,.06)' }}>
          {out.text && (
            <span data-testid="reply-out" className="select-text" style={{ font: fnt(500, 15), color: '#ece6dc' }}>
              {out.text}
            </span>
          )}
          <span style={{ font: fnt(400, 11, 'sans', 1.45), color: out.error ? '#ff8a7a' : '#7fe0a8' }}>{out.note}</span>
        </div>
      )}
    </div>
  );
}
