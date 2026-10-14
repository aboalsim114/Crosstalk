import { useEffect, useState } from 'react';
import { isTauri, recapFiles } from '../../shared/ipc';
import { Toggle, fnt } from '../../shared/ui/atoms';
import { renderRecapPng } from './card';
import { pickQuotes, recapFacts, useRecap, type MatchStats } from './recap';
import { useApp } from '../../app/state/store';

/** Copies a PNG to the clipboard (paste into Discord with Ctrl+V). */
async function copyPng(png: Blob): Promise<boolean> {
  try {
    await navigator.clipboard.write([new ClipboardItem({ 'image/png': png })]);
    return true;
  } catch {
    return false;
  }
}

const dateFr = (ms: number) => new Date(ms).toLocaleDateString('fr-FR', { day: 'numeric', month: 'long', year: 'numeric' });

/** End-of-match recap: a card to save or paste into Discord / X. */
export function RecapPanel() {
  const { current, last, unseen } = useRecap();
  const flash = useApp.getState().flash;
  const [which, setWhich] = useState<'last' | 'current'>(last ? 'last' : 'current');
  const [quotes, setQuotes] = useState(false);
  const [url, setUrl] = useState<string | null>(null);
  const [png, setPng] = useState<Blob | null>(null);
  const [saved, setSaved] = useState<string | null>(null);
  const match: MatchStats | null = which === 'last' ? (last ?? current) : (current ?? last);
  const live = match === current && !!current;

  useEffect(() => {
    if (unseen) useRecap.setState({ unseen: false });
    if (last) setWhich('last');
  }, [unseen, last]);

  useEffect(() => {
    if (!match || match.translated + match.voice + match.chat === 0) {
      setPng(null);
      setUrl(null);
      return;
    }
    let gone = false;
    let made: string | null = null;
    renderRecapPng(recapFacts(match), { quotes: quotes ? pickQuotes(match, 2) : [], date: dateFr(match.endedAt ?? Date.now()) }).then(
      (b) => {
        if (gone) return;
        made = URL.createObjectURL(b);
        setPng(b);
        setUrl(made);
        setSaved(null);
      },
      (e) => console.warn('recap image failed', e),
    );
    return () => {
      gone = true;
      if (made) URL.revokeObjectURL(made);
    };
  }, [match, quotes]);

  const copy = () => png && copyPng(png).then((ok) => flash(ok ? 'Image copiée : collez-la dans Discord avec Ctrl+V' : 'Copie impossible : enregistrez l’image'));
  const save = () =>
    png &&
    recapFiles.save(png).then(
      (path) => {
        setSaved(path);
        flash(`Enregistré : ${path}`);
      },
      (e) => flash(`Enregistrement impossible : ${e}`),
    );

  return (
    <div className="glass flex flex-col gap-4 p-5" data-testid="recap-panel">
      <div className="flex items-start justify-between gap-4">
        <div className="flex flex-col gap-1">
          <span style={{ font: fnt(400, 22, 'serif'), color: '#ece6dc' }}>Résumé de partie</span>
          <span style={{ font: fnt(400, 12, 'sans', 1.5), color: '#6e6961' }}>
            À la fin de chaque partie, Crosstalk prépare une image à partager : carte, score, annonces traduites. Sans aucun pseudo. Rien n’est envoyé : c’est vous qui la partagez.
          </span>
        </div>
        {current && last && (
          <div className="flex flex-none gap-1.5">
            {(['last', 'current'] as const).map((w) => (
              <span key={w} className="pill px-3 py-1.5" role="button" onClick={() => setWhich(w)} style={{ font: fnt(500, 11), color: which === w ? '#ffa24a' : '#8f8a83' }}>
                {w === 'last' ? 'Dernière partie' : 'En cours'}
              </span>
            ))}
          </div>
        )}
      </div>
      {!match || !url ? (
        <span data-testid="recap-empty" style={{ font: fnt(400, 12.5, 'sans', 1.5), color: '#6e6961' }}>
          {current ? 'Partie en cours : le résumé apparaît dès la première ligne traduite.' : 'Aucune partie pour l’instant. Démarrez une session : le résumé sera prêt à la fin de la partie (fin de match signalée par CS2 si le nom de la carte est activé, sinon à l’arrêt de la session).'}
        </span>
      ) : (
        <>
          <img src={url} alt="Résumé de partie" data-testid="recap-image" style={{ width: '100%', maxWidth: 720, borderRadius: 10, border: '1px solid rgba(255,255,255,.07)' }} />
          {live && <span style={{ font: fnt(400, 11.5), color: '#ffa24a' }}>Aperçu de la partie en cours : l’image finale sera prête à la fin du match.</span>}
          <div className="flex flex-wrap items-center gap-2.5">
            <div className="pill px-3.5 py-2" role="button" onClick={copy} style={{ font: fnt(500, 12) }}>
              Copier l’image
            </div>
            {isTauri && (
              <div className="pill px-3.5 py-2" role="button" onClick={save} style={{ font: fnt(500, 12) }}>
                Enregistrer dans Documents
              </div>
            )}
            {saved && (
              <div className="pill px-3.5 py-2" role="button" onClick={() => recapFiles.reveal(saved).catch(() => {})} style={{ font: fnt(500, 12), color: '#8f8a83' }}>
                Afficher le fichier
              </div>
            )}
            <div className="flex-1" />
            <span style={{ font: fnt(400, 12), color: '#8f8a83' }}>Ajouter 2 phrases traduites</span>
            <Toggle on={quotes} onClick={() => setQuotes(!quotes)} label="Ajouter 2 phrases traduites" />
          </div>
        </>
      )}
    </div>
  );
}
