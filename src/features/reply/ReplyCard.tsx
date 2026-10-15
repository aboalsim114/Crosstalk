import { useEffect, useState } from 'react';
import { reply, type ReplyLanguage, type ReplyTarget } from '../../shared/ipc';
import { fnt } from '../../shared/ui/atoms';
import { ReplyBox, teammatesLanguage } from './ReplyBox';
import { recentLangs } from '../languages/langs';
import { useApp } from '../../app/state/store';

/** "Réponds dans leur langue": the same box as the in-game pop-up. */
export function ReplyCard() {
  const feed = useApp((s) => s.feed);
  const langKey = useApp((s) => `${s.target}|${Object.entries(s.src).filter(([, on]) => on).map(([l]) => l).join(',')}|${s.llm.enabled}|${s.llm.model}`);
  const [target, setTarget] = useState<ReplyTarget>(() => teammatesLanguage(recentLangs(feed)));
  const [langs, setLangs] = useState<ReplyLanguage[]>([]);
  useEffect(() => {
    const load = () =>
      reply.models().then((m) => {
        setLangs(m);
        const targets = m.map((x) => x.target);
        setTarget((t) => (targets.includes(t) ? t : teammatesLanguage(recentLangs(useApp.getState().feed), targets)));
      });
    // The Rust side follows the new languages and engine a moment later.
    const timer = setTimeout(load, 600);
    return () => clearTimeout(timer);
  }, [langKey]);
  const problem = langs.find((l) => !l.ready)?.problem ?? null;
  return (
    <div className="glass flex flex-col gap-3.5 p-5" data-testid="reply-card">
      <div className="flex items-baseline justify-between">
        <span className="lbl">RÉPONDRE DANS LEUR LANGUE</span>
        <span style={{ font: fnt(400, 10, 'mono'), color: '#5a5650' }}>en jeu : Ctrl+Alt+F7</span>
      </div>
      <span style={{ font: fnt(400, 11.5, 'sans', 1.5), color: '#6e6961' }}>
        Écrivez dans votre langue : le moteur de « Traduction IA » traduit et Crosstalk copie le message. Dans CS2, ouvrez le chat (Y ou U), collez (Ctrl+V), Entrée. Rien n’est tapé dans le jeu à votre place.
      </span>
      {problem && (
        <div className="rounded-xl px-3 py-2.5" style={{ background: 'rgba(255,162,74,.07)', border: '1px solid rgba(255,162,74,.22)', font: fnt(400, 11.5, 'sans', 1.45), color: '#ffc68a' }} data-testid="reply-unavailable">
          Texte libre indisponible : {problem}. Les phrases CS toutes prêtes fonctionnent toujours.
        </div>
      )}
      <ReplyBox target={target} onTarget={setTarget} />
    </div>
  );
}
