import { useEffect, useRef, useState } from 'react';
import { dictation, isTauri, reply, voice, type ReplyPhrase, type ReplyTarget } from '../../shared/ipc';
import { fnt } from '../../shared/ui/atoms';
import { NATIVE_NAME, isPlayerLang } from '../languages/langs';

/** A language in its own words ("English", "Русский"…). */
export const targetName = (t: string) => (isPlayerLang(t) ? NATIVE_NAME[t] : t.toUpperCase());

/** Where replies can go and the user's language, from the Rust core (browser preview: French into en / ru). */
export function useReplyLanguages(reloadKey = 0): { me: string; targets: ReplyTarget[] } {
  const [langs, setLangs] = useState<{ me: string; targets: ReplyTarget[] }>({ me: 'fr', targets: isTauri ? [] : ['en', 'ru'] });
  useEffect(() => {
    if (!isTauri) return;
    reply.models().then((m) => setLangs({ me: m[0]?.source ?? 'fr', targets: m.map((x) => x.target) }));
  }, [reloadKey]);
  return langs;
}

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

/** Teammates' language from the recent lines (voice and chat): the most heard of the reply languages. */
export function teammatesLanguage(langs: string[], targets: ReplyTarget[] = ['en', 'ru']): ReplyTarget {
  let best: ReplyTarget | null = null;
  let most = 0;
  for (const t of targets) {
    const n = langs.filter((l) => l === t).length;
    if (n > most) [best, most] = [t, n];
  }
  return best ?? (targets.includes('en') ? 'en' : (targets[0] ?? 'en'));
}

export const PASTE_HINT = 'Copié ✓ — dans CS2 : Y (tous) ou U (équipe), Ctrl+V, Entrée';

interface Props {
  target: ReplyTarget;
  onTarget: (t: ReplyTarget) => void;
  /** Called once the text is in the clipboard. */
  onCopied?: () => void;
  autoFocusKey?: number;
  /** In the pop-up: with "Dire mes réponses écrites", each reply is also said in the game. */
  speakOnSend?: boolean;
}

/** The user's language in, the teammates' language out, copied to the clipboard. */
export function ReplyBox({ target, onTarget, onCopied, autoFocusKey, speakOnSend }: Props) {
  const { me, targets } = useReplyLanguages(autoFocusKey);
  const myName = targetName(me);
  const [text, setText] = useState('');
  const [phrases, setPhrases] = useState<ReplyPhrase[]>([]);
  const [busy, setBusy] = useState(false);
  const [out, setOut] = useState<{ text: string; note: string; error?: boolean } | null>(null);
  const input = useRef<HTMLInputElement>(null);
  // AI voice ready (on, cable fed): the translation can be said in the game.
  const [voiceReady, setVoiceReady] = useState(false);
  // "Dire mes réponses écrites": read at each opening of the pop-up.
  const [speakTyped, setSpeakTyped] = useState(false);

  useEffect(() => {
    reply.phrases().then(setPhrases);
    voice.status().then((st) => {
      setVoiceReady(!!st?.options.enabled && st.routing);
      setSpeakTyped(!!st?.options.speakTyped);
    });
  }, []);
  useEffect(() => {
    input.current?.focus();
  }, [autoFocusKey]);

  const done = async (translated: string, note: string) => {
    const ok = await copyText(translated);
    setOut({ text: translated, note: ok ? `${note} · ${PASTE_HINT}` : 'Copie impossible : sélectionnez le texte ci-dessus.' });
    if (speakOnSend && speakTyped && voiceReady) {
      // The pop-up closes (CS2 back in front for the voice key), then the voice speaks.
      try {
        await reply.say(translated, target);
      } catch (e) {
        setOut({ text: translated, note: String(e), error: true });
        return;
      }
    }
    if (ok) onCopied?.();
  };
  const send = async () => {
    const t = text.trim();
    if (!t || busy) return;
    setBusy(true);
    setOut({ text: '…', note: 'Traduction locale en cours (le premier message charge le modèle, quelques secondes)' });
    try {
      const r = await reply.translate(t, target);
      await done(r.text, r.via === 'phrasebook' ? 'Phrase CS vérifiée' : `Traduit par le LLM en ${(r.ms / 1000).toFixed(1).replace('.', ',')} s`);
      setText('');
    } catch (e) {
      setOut({ text: '', note: String(e), error: true });
    } finally {
      setBusy(false);
    }
  };
  const say = async (t: string) => {
    try {
      const ms = await voice.say(t, target);
      setOut((o) => o && { ...o, note: `Dit en jeu par la voix IA (${(ms / 1000).toFixed(1).replace('.', ',')} s)` });
    } catch (e) {
      setOut((o) => o && { ...o, note: String(e), error: true });
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
        {targets.length === 0 && <span style={{ font: fnt(400, 11.5), color: '#8f8a83' }}>aucune langue de coéquipier (Langues › Mes langues)</span>}
        {targets.map((t) => (
          <div
            key={t}
            role="button"
            aria-pressed={target === t}
            onClick={() => onTarget(t)}
            className={`pill px-3 py-1 ${target === t ? 'pill-ember' : ''}`}
            style={{ font: fnt(500, 11.5) }}
          >
            {targetName(t)}
          </div>
        ))}
      </div>
      <div className="flex gap-2">
        <input
          ref={input}
          className="text-input"
          aria-label={`Message (${myName})`}
          placeholder={`Écrivez en ${myName}, puis Entrée…`}
          value={text}
          maxLength={300}
          disabled={busy}
          onChange={(e) => setText(e.target.value)}
          onKeyDown={(e) => e.key === 'Enter' && send()}
          style={{ flex: 1, minWidth: 0 }}
        />
        {isTauri && (
          <div className="pill flex-none px-3 py-2" role="button" title={`Dicter en ${myName} (Alt Gr)`} onClick={() => void dictation.toggle([], target)} style={{ font: fnt(500, 12) }}>
            🎙 Dicter
          </div>
        )}
      </div>
      {me === 'fr' && phrases.length > 0 && (
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
          {voiceReady && out.text && out.text !== '…' && (
            <div className="pill self-start px-3 py-1" role="button" onClick={() => say(out.text)} style={{ font: fnt(500, 11.5) }}>
              🔊 Dire en jeu
            </div>
          )}
        </div>
      )}
    </div>
  );
}
