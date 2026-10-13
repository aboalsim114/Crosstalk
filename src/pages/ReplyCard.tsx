import { useEffect, useState } from 'react';
import { modelDownload, reply, type ReplyModel, type ReplyTarget } from '../backend';
import { fnt } from '../components/atoms';
import { ReplyBox, teammatesLanguage } from '../reply/ReplyBox';
import { useApp } from '../state/store';

/** Recent languages heard or read (voice and chat), newest first. */
export function recentLangs(feed: { src: string; tid?: number }[], n = 20): string[] {
  return feed
    .filter((f) => f.tid != null)
    .slice(-n)
    .map((f) => f.src);
}

/** "Réponds dans leur langue": the same box as the in-game pop-up. */
export function ReplyCard() {
  const feed = useApp((s) => s.feed);
  const model = useApp((s) => s.model);
  const flash = useApp((s) => s.flash);
  const [target, setTarget] = useState<ReplyTarget>(() => teammatesLanguage(recentLangs(feed)));
  const [models, setModels] = useState<ReplyModel[]>([]);
  useEffect(() => {
    reply.models().then(setModels);
    const un = modelDownload.onProgress((s) => s.state === 'done' && reply.models().then(setModels));
    return () => void un.then((f) => f());
  }, []);
  const missing = models.filter((m) => !m.installed);
  const download = () => modelDownload.start(model, true).then(() => flash('Téléchargement des modèles de réponse lancé (voir Audio & IA)'));
  return (
    <div className="glass flex flex-col gap-3.5 p-5" data-testid="reply-card">
      <div className="flex items-baseline justify-between">
        <span className="lbl">RÉPONDRE DANS LEUR LANGUE</span>
        <span style={{ font: fnt(400, 10, 'mono'), color: '#5a5650' }}>en jeu : Ctrl+Alt+F7</span>
      </div>
      <span style={{ font: fnt(400, 11.5, 'sans', 1.5), color: '#6e6961' }}>
        Écrivez en français : Crosstalk traduit sur ce PC et copie le message. Dans CS2, ouvrez le chat (Y ou U), collez (Ctrl+V), Entrée. Rien n’est tapé dans le jeu à votre place.
      </span>
      {missing.length > 0 && (
        <div className="flex items-center justify-between gap-3 rounded-xl px-3 py-2.5" style={{ background: 'rgba(255,162,74,.07)', border: '1px solid rgba(255,162,74,.22)' }}>
          <span style={{ font: fnt(400, 11.5, 'sans', 1.45), color: '#ffc68a' }}>
            Phrases CS disponibles. Pour le texte libre, téléchargez {missing.map((m) => `français → ${m.target.toUpperCase()}`).join(' et ')} ({missing.reduce((a, m) => a + m.sizeMb, 0)} Mo).
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
