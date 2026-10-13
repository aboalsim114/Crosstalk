import { useEffect, useState } from 'react';
import { reply, type ReplyTarget } from '../backend';
import { fnt } from '../components/atoms';
import { ReplyBox } from './ReplyBox';

/** Pop-up over the game (Ctrl+Alt+F7): type, Enter, it is in the clipboard. */
export function ReplyWindow() {
  const [target, setTarget] = useState<ReplyTarget>('en');
  const [opened, setOpened] = useState(0);
  useEffect(() => {
    const un = reply.onOpen((t) => {
      setTarget(t);
      setOpened((n) => n + 1);
    });
    const esc = (e: KeyboardEvent) => e.key === 'Escape' && reply.hide();
    window.addEventListener('keydown', esc);
    return () => {
      void un.then((f) => f());
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
      <ReplyBox key={opened} target={target} onTarget={setTarget} autoFocusKey={opened} onCopied={() => setTimeout(() => reply.hide(), 1800)} />
    </div>
  );
}
