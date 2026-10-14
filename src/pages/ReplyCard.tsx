import { useEffect, useState } from 'react';
import { modelDownload, reply, type ReplyModel, type ReplyTarget } from '../backend';
import { fnt } from '../components/atoms';
import { ReplyBox, targetName, teammatesLanguage } from '../reply/ReplyBox';
import { recentLangs } from '../state/langs';
import { setupNeeds } from '../state/setup';
import { useApp } from '../state/store';

/** "Réponds dans leur langue": the same box as the in-game pop-up. */
export function ReplyCard() {
  const feed = useApp((s) => s.feed);
  const model = useApp((s) => s.model);
  const flash = useApp((s) => s.flash);
  const langKey = useApp((s) => `${s.target}|${Object.entries(s.src).filter(([, on]) => on).map(([l]) => l).join(',')}`);
  const [target, setTarget] = useState<ReplyTarget>(() => teammatesLanguage(recentLangs(feed)));
  const [models, setModels] = useState<ReplyModel[]>([]);
  useEffect(() => {
    const load = () =>
      reply.models().then((m) => {
        setModels(m);
        const targets = m.map((x) => x.target);
        setTarget((t) => (targets.includes(t) ? t : teammatesLanguage(recentLangs(useApp.getState().feed), targets)));
      });
    // The Rust side follows the new languages a moment later.
    const timer = setTimeout(load, 400);
    const un = modelDownload.onProgress((s) => s.state === 'done' && load());
    return () => {
      clearTimeout(timer);
      void un.then((f) => f());
    };
  }, [langKey]);
  const missing = models.filter((m) => !m.installed);
  const download = () => {
    const n = setupNeeds(useApp.getState());
    return modelDownload.start(model, true, n.plan).then(() => flash('Téléchargement des modèles de réponse lancé (voir Audio & IA)'));
  };
  return (
    <div className="glass flex flex-col gap-3.5 p-5" data-testid="reply-card">
      <div className="flex items-baseline justify-between">
        <span className="lbl">RÉPONDRE DANS LEUR LANGUE</span>
        <span style={{ font: fnt(400, 10, 'mono'), color: '#5a5650' }}>en jeu : Ctrl+Alt+F7</span>
      </div>
      <span style={{ font: fnt(400, 11.5, 'sans', 1.5), color: '#6e6961' }}>
        Écrivez dans votre langue : Crosstalk traduit sur ce PC et copie le message. Dans CS2, ouvrez le chat (Y ou U), collez (Ctrl+V), Entrée. Rien n’est tapé dans le jeu à votre place.
      </span>
      {missing.length > 0 && (
        <div className="flex items-center justify-between gap-3 rounded-xl px-3 py-2.5" style={{ background: 'rgba(255,162,74,.07)', border: '1px solid rgba(255,162,74,.22)' }}>
          <span style={{ font: fnt(400, 11.5, 'sans', 1.45), color: '#ffc68a' }}>
            Pour le texte libre, téléchargez {missing.map((m) => `${targetName(m.source)} → ${targetName(m.target)}`).join(', ')} ({missing.reduce((a, m) => a + m.sizeMb, 0)} Mo).
          </span>
          <div className="pill pill-ember flex-none px-3.5 py-1.5" role="button" onClick={download} style={{ font: fnt(500, 12) }}>
            Télécharger
          </div>
        </div>
      )}
      <ReplyBox target={target} onTarget={setTarget} />
    </div>
  );
}
