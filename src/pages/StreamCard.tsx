import { useEffect, useState } from 'react';
import { streamOverlay, type StreamStatus } from '../backend';
import { Slider, ToggleRow, fnt } from '../components/atoms';
import { copyText } from '../reply/ReplyBox';
import { TEST_LINE } from '../state/stream';
import { useApp } from '../state/store';

/** What the card says about the OBS page. */
export function streamSummary(st: StreamStatus | null, enabled: boolean): { value: string; detail: string; color: string } {
  if (!enabled) return { value: 'Désactivé', detail: 'Aucune page n’est servie, rien n’est envoyé.', color: '#6e6961' };
  if (!st) return { value: 'Démarrage…', detail: '', color: '#6e6961' };
  if (st.error) return { value: 'Indisponible', detail: st.error, color: '#ff8a7a' };
  if (!st.listening) return { value: 'Démarrage…', detail: '', color: '#ffa24a' };
  if (!st.clients) return { value: 'Prêt · en attente d’OBS', detail: 'Ajoutez la source navigateur ci-dessous.', color: '#ffa24a' };
  return {
    value: `OBS connecté${st.clients > 1 ? ` · ${st.clients} sources` : ''}`,
    detail: `${st.sent} sous-titre${st.sent > 1 ? 's' : ''} envoyé${st.sent > 1 ? 's' : ''} depuis le lancement.`,
    color: '#7fe0a8',
  };
}

/** Mode streamer: translated subtitles as an OBS browser source. */
export function StreamCard() {
  const opts = useApp((s) => s.stream);
  const { setStream, flash } = useApp.getState();
  const [st, setSt] = useState<StreamStatus | null>(null);
  useEffect(() => {
    const load = () => streamOverlay.status().then(setSt);
    load();
    const t = setInterval(load, 2000);
    return () => clearInterval(t);
  }, []);
  const sum = streamSummary(st, opts.enabled);
  const url = st?.url ?? 'http://127.0.0.1:31983/';
  return (
    <div className="glass flex flex-col gap-4 p-5" data-testid="stream-card">
      <div className="flex items-baseline justify-between">
        <span className="lbl">MODE STREAMER · SOUS-TITRES POUR OBS</span>
        <span style={{ font: fnt(400, 10, 'mono'), color: '#5a5650' }}>local · 127.0.0.1</span>
      </div>
      <div className="grid gap-5" style={{ gridTemplateColumns: 'minmax(0,1fr) minmax(0,1fr)' }}>
        <div className="flex flex-col gap-3">
          <ToggleRow title="Afficher les sous-titres sur le stream" sub="Vos viewers lisent les annonces en français" on={opts.enabled} onClick={() => setStream({ enabled: !opts.enabled })} />
          <div className="flex items-start gap-2.5">
            <div className="mt-[7px] h-1.5 w-1.5 flex-none rounded-[3px]" style={{ background: sum.color, boxShadow: `0 0 8px ${sum.color}` }} />
            <div className="flex min-w-0 flex-col gap-0.5">
              <span data-testid="stream-state" style={{ font: fnt(500, 13), color: '#ece6dc' }}>
                {sum.value}
              </span>
              <span style={{ font: fnt(400, 11, 'sans', 1.45), color: '#6e6961' }}>{sum.detail}</span>
            </div>
          </div>
          <div className="flex gap-2">
            <input className="text-input min-w-0 flex-1" readOnly value={url} aria-label="Adresse de la source navigateur" style={{ fontFamily: 'var(--font-mono)' }} />
            <div className="pill px-3.5 py-2" role="button" onClick={() => copyText(url).then((ok) => flash(ok ? 'Adresse copiée' : 'Copie impossible'))} style={{ font: fnt(500, 12) }}>
              Copier
            </div>
          </div>
          <span style={{ font: fnt(400, 11, 'sans', 1.55), color: '#6e6961' }}>
            Dans OBS : Sources › + › <b>Navigateur</b> › URL ci-dessus, largeur 1920, hauteur 1080. Placez la source au-dessus de la capture du jeu. Seuls les sous-titres finaux sont envoyés.
          </span>
          <div className="pill self-start px-3.5 py-2" role="button" aria-disabled={!opts.enabled} onClick={() => opts.enabled && streamOverlay.push(TEST_LINE)} style={{ font: fnt(500, 12) }}>
            Envoyer une ligne de test
          </div>
        </div>
        <div className="flex flex-col gap-3">
          <ToggleRow title="Afficher les pseudos" sub="Désactivé par défaut (données personnelles)" on={opts.names} onClick={() => setStream({ names: !opts.names })} />
          <ToggleRow title="Afficher la phrase originale" sub="Sous la traduction" on={opts.original} onClick={() => setStream({ original: !opts.original })} />
          <ToggleRow title="Mention « Crosstalk »" sub="Petit texte dans le coin" on={opts.watermark} onClick={() => setStream({ watermark: !opts.watermark })} />
          <Slider label="Taille du texte" unit="px" min={18} max={72} value={opts.size} onChange={(v) => setStream({ size: v })} />
          <Slider label="Durée d’affichage" unit="s" min={3} max={20} value={opts.duration} onChange={(v) => setStream({ duration: v })} />
          <Slider label="Lignes visibles" unit="" min={1} max={5} value={opts.max} onChange={(v) => setStream({ max: v })} />
        </div>
      </div>
    </div>
  );
}
