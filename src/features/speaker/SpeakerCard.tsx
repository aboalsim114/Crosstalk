import { useEffect, useState } from 'react';
import { speakerId, type SpeakerTest } from '../../shared/ipc';
import { Slider, ToggleRow, fnt } from '../../shared/ui/atoms';
import { speakerSummary, useSpeakerStatus } from './speaker';
import { useApp } from '../../app/state/store';

const TEST_DELAY_S = 4;

/** What the test result tells the user. */
export function testVerdict(t: SpeakerTest | null): { text: string; color: string } | null {
  if (!t) return null;
  if (t.error) return { text: t.error, color: '#ff8a7a' };
  if (!t.foreground)
    return {
      text: 'CS2 n’était pas au premier plan : retournez dans CS2 avant la fin du décompte.',
      color: '#ffa24a',
    };
  if (t.blank)
    return {
      text: 'Image noire : passez CS2 en « plein écran fenêtré ».',
      color: '#ff8a7a',
    };
  if (t.names.length) return { text: `Lu : ${t.names.join(', ')}`, color: '#7fe0a8' };
  if (t.lines.length)
    return {
      text: `Texte lu, mais aucun pseudo reconnu : ${t.lines.join(' · ')}`,
      color: '#ffa24a',
    };
  return {
    text: 'Aucun texte lu : ajustez la zone pour qu’elle couvre la liste des joueurs qui parlent.',
    color: '#ffa24a',
  };
}

/** "Qui parle ?": the speaker's name on voice lines, read on CS2's screen. */
export function SpeakerCard() {
  const opts = useApp((s) => s.speaker);
  const ownName = useApp((s) => s.ownName);
  const { setSpeaker, set } = useApp.getState();
  const st = useSpeakerStatus((s) => s.status);
  const [count, setCount] = useState(0);
  const [test, setTest] = useState<SpeakerTest | null>(null);
  useEffect(() => {
    if (count <= 0) return;
    const t = setTimeout(() => setCount((c) => c - 1), 1000);
    return () => clearTimeout(t);
  }, [count]);
  const sum = speakerSummary(st, opts.enabled);
  const verdict = testVerdict(test);
  const r = opts.region;
  const pct = (v: number) => Math.round(v * 100);
  const setRegion = (p: Partial<typeof r>) => setSpeaker({ region: { ...r, ...p } });
  const runTest = () => {
    if (count > 0) return;
    setTest(null);
    setCount(TEST_DELAY_S);
    speakerId.test(r, TEST_DELAY_S * 1000).then((t) => {
      setTest(t);
      setCount(0);
    });
  };
  return (
    <div className="glass flex flex-col gap-4 p-5" data-testid="speaker-card">
      <div className="flex items-baseline justify-between">
        <span className="lbl">QUI PARLE ? · EXPÉRIMENTAL</span>
        <span style={{ font: fnt(400, 10, 'mono'), color: '#5a5650' }}>OCR Windows · local</span>
      </div>
      <ToggleRow title="Afficher le pseudo de celui qui parle" sub="« Sasha : deux en B » : lu sur l’indicateur de voix de CS2" on={opts.enabled} onClick={() => setSpeaker({ enabled: !opts.enabled })} />
      <div className="flex items-start gap-2.5">
        <div className="mt-[7px] h-1.5 w-1.5 flex-none rounded-[3px]" style={{ background: sum.color, boxShadow: `0 0 8px ${sum.color}` }} />
        <div className="flex min-w-0 flex-col gap-0.5">
          <span data-testid="speaker-state" style={{ font: fnt(500, 13), color: '#ece6dc' }}>
            {sum.value}
          </span>
          <span style={{ font: fnt(400, 11, 'sans', 1.45), color: '#6e6961' }}>{sum.detail}</span>
        </div>
      </div>
      {opts.enabled && st?.known.length ? (
        <div className="flex flex-wrap items-center gap-1.5" data-testid="speaker-known">
          <span style={{ font: fnt(400, 11), color: '#6e6961' }}>Pseudos lus (cliquez sur le vôtre) :</span>
          {st.known.map((n) => (
            <span
              key={n}
              className="pill px-2.5 py-1"
              role="button"
              title="C’est moi : ne pas l’afficher"
              onClick={() => set({ ownName: n })}
              style={{
                font: fnt(500, 11),
                color: n === ownName ? '#5a5650' : '#ece6dc',
              }}
            >
              {n}
            </span>
          ))}
        </div>
      ) : null}
      <div className="grid gap-x-5 gap-y-3" style={{ gridTemplateColumns: 'minmax(0,1fr) minmax(0,1fr)' }}>
        <Slider label="Zone lue · gauche" unit="%" min={0} max={90} value={pct(r.x)} onChange={(v) => setRegion({ x: v / 100 })} />
        <Slider label="Zone lue · haut" unit="%" min={0} max={90} value={pct(r.y)} onChange={(v) => setRegion({ y: v / 100 })} />
        <Slider label="Largeur" unit="%" min={5} max={60} value={pct(r.w)} onChange={(v) => setRegion({ w: v / 100 })} />
        <Slider label="Hauteur" unit="%" min={5} max={60} value={pct(r.h)} onChange={(v) => setRegion({ h: v / 100 })} />
      </div>
      <div className="flex items-center gap-3">
        <div className="pill self-start px-3.5 py-2" role="button" aria-disabled={count > 0} onClick={runTest} style={{ font: fnt(500, 12) }} data-testid="speaker-test">
          {count > 0 ? `Retournez dans CS2… ${count}` : 'Tester la zone'}
        </div>
        <span style={{ font: fnt(400, 11, 'sans', 1.45), color: '#6e6961' }}>Cliquez, passez dans CS2 et maintenez votre touche micro : votre pseudo doit apparaître.</span>
      </div>
      {test?.preview && (
        <img
          src={test.preview}
          alt="Zone lue"
          data-testid="speaker-preview"
          style={{
            maxWidth: '100%',
            maxHeight: 180,
            alignSelf: 'flex-start',
            borderRadius: 8,
            border: '1px solid rgba(255,255,255,.08)',
            imageRendering: 'pixelated',
          }}
        />
      )}
      {verdict && (
        <span data-testid="speaker-verdict" style={{ font: fnt(500, 12, 'sans', 1.45), color: verdict.color }}>
          {verdict.text}
        </span>
      )}
      <span style={{ font: fnt(400, 11, 'sans', 1.55), color: '#6e6961' }}>
        Crosstalk lit seulement cette zone de votre écran, comme une capture d’écran, quand quelqu’un parle et que CS2 est au premier plan. Aucun accès au jeu. Les images ne sont jamais enregistrées, et les pseudos restent en mémoire jusqu’à la fermeture. CS2
        doit être en « plein écran fenêtré ». Les pseudos en cyrillique demandent la langue russe installée dans Windows.
      </span>
    </div>
  );
}
