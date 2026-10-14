import { dictation, type DictationState } from '../backend';
import { fnt } from '../components/atoms';
import { PASTE_HINT, targetName } from './ReplyBox';

/** What the dictation panel says, from the Rust state. */
export function dictationView(st: DictationState): { title: string; detail: string; color: string } {
  switch (st.state) {
    case 'listening':
      return { title: 'Parlez dans votre langue…', detail: `${st.seconds.toFixed(1).replace('.', ',')} s · la phrase se termine toute seule (ou Ctrl+Alt+F6)`, color: '#ff8a7a' };
    case 'transcribing':
      return { title: 'Transcription et traduction…', detail: `Sur ce PC, vers ${targetName(st.target)}`, color: '#ffa24a' };
    case 'done':
      return {
        title: st.translated ?? '',
        detail: st.copied ? `${st.via === 'phrasebook' ? 'Phrase CS vérifiée' : 'Traduit sur ce PC'} · ${PASTE_HINT}` : 'Copie impossible : sélectionnez le texte.',
        color: '#7fe0a8',
      };
    case 'error':
      return { title: 'Dictée impossible', detail: st.error ?? '', color: '#ff8a7a' };
  }
}

/** Spoken reply in progress (Ctrl+Alt+F6 or "Dicter"). */
export function DictationPanel({ st }: { st: DictationState }) {
  const v = dictationView(st);
  return (
    <div className="flex flex-col gap-2 rounded-xl px-3 py-3" data-testid="dictation" style={{ background: 'rgba(0,0,0,.35)', border: '1px solid rgba(255,255,255,.06)' }}>
      <div className="flex items-center gap-2.5">
        <div className="h-2 w-2 flex-none rounded-full" style={{ background: v.color, boxShadow: `0 0 10px ${v.color}` }} />
        <span className="select-text" style={{ font: fnt(500, st.state === 'done' ? 16 : 13.5), color: '#ece6dc' }}>
          {v.title}
        </span>
      </div>
      {st.state === 'listening' && (
        <div className="h-1.5 overflow-hidden rounded-full" style={{ background: 'rgba(255,255,255,.08)' }}>
          <div className="h-full rounded-full" style={{ width: `${Math.round(st.level * 100)}%`, background: '#ff8a7a', transition: 'width .1s' }} />
        </div>
      )}
      {st.text && st.state !== 'listening' && (
        <span style={{ font: fnt(400, 11.5, 'sans', 1.45), color: '#8f8a83', fontStyle: 'italic' }}>« {st.text} »</span>
      )}
      <span style={{ font: fnt(400, 11, 'sans', 1.45), color: st.state === 'error' ? '#ff8a7a' : '#6e6961' }}>{v.detail}</span>
      {st.state === 'listening' && (
        <div className="pill self-start px-3 py-1" role="button" onClick={() => dictation.toggle([])} style={{ font: fnt(500, 11.5) }}>
          Terminer
        </div>
      )}
    </div>
  );
}
