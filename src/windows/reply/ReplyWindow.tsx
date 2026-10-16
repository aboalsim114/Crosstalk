import { useEffect, useRef, useState } from 'react';
import { dictation, reply, type DictationState, type ReplyTarget } from '../../shared/ipc';
import { fnt } from '../../shared/ui/atoms';
import { DictationPanel } from '../../features/reply/DictationPanel';
import { ReplyBox } from '../../features/reply/ReplyBox';

/** Pop-up over the game (Mouse 4 / Ctrl+Alt+F7): type, Enter, it is in the clipboard. */
export function ReplyWindow() {
  const [target, setTarget] = useState<ReplyTarget>('en');
  const [opened, setOpened] = useState(0);
  const [dict, setDict] = useState<DictationState | null>(null);
  // Read by the Escape key, outside React's render: never inside a state updater.
  const dictRef = useRef<DictationState | null>(null);
  dictRef.current = dict;
  // One pending "back to the game" at most; reopening the window cancels it.
  const hideTimer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  const hideLater = (ms: number) => {
    clearTimeout(hideTimer.current);
    hideTimer.current = setTimeout(() => void reply.hide(), ms);
  };
  useEffect(() => {
    const un = reply.onOpen((t) => {
      clearTimeout(hideTimer.current);
      setTarget(t);
      setDict(null);
      setOpened((n) => n + 1);
    });
    const unDict = dictation.onState((st) => {
      setDict(st);
      setTarget(st.target);
      // A spoken reply copied: back to the game.
      if (st.state === 'done' && st.copied) hideLater(2500);
    });
    const esc = (e: KeyboardEvent) => {
      if (e.key !== 'Escape') return;
      if (dictRef.current?.state === 'listening') void dictation.toggle([]);
      else void reply.hide();
    };
    window.addEventListener('keydown', esc);
    return () => {
      clearTimeout(hideTimer.current);
      void un.then((f) => f());
      void unDict.then((f) => f());
      window.removeEventListener('keydown', esc);
    };
  }, []);
  return (
    <div className="glass m-2 flex flex-col gap-3 p-4" style={{ background: 'rgba(14,13,12,.92)' }}>
      <div className="flex items-center justify-between" data-tauri-drag-region>
        <span style={{ font: fnt(400, 18, 'serif'), color: '#ece6dc' }} data-tauri-drag-region>
          Répondre dans leur langue
        </span>
        <span style={{ font: fnt(400, 10, 'mono'), color: '#5a5650' }}>Échap pour fermer</span>
      </div>
      {dict && <DictationPanel st={dict} />}
      <ReplyBox key={opened} target={target} onTarget={setTarget} autoFocusKey={opened} onCopied={() => hideLater(1800)} speakOnSend />
    </div>
  );
}
