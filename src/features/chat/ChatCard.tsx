import { chat } from '../../shared/ipc';
import { ToggleRow, fnt } from '../../shared/ui/atoms';
import { chatCounts, cs2SetupSummary, useChat } from './chat';
import { useApp } from '../../app/state/store';

/** Written chat: where console.log is, whether -condebug is set, manual path, own name. */
export function ChatCard() {
  const s = useApp();
  const { native, status } = useChat();
  const sum = cs2SetupSummary(status);
  return (
    <div className="glass flex flex-col gap-3.5 p-5" data-testid="chat-card">
      <div className="flex items-baseline justify-between">
        <span className="lbl">CHAT ÉCRIT · CONSOLE.LOG</span>
        {native && status && <span style={{ font: fnt(400, 10, 'mono'), color: '#5a5650' }}>règles {status.rulesVersion}</span>}
      </div>
      {!native ? (
        <span style={{ font: fnt(400, 11.5, 'sans', 1.5), color: '#6e6961' }}>Dans l’application Windows, Crosstalk lit le chat de CS2 dans console.log (option de lancement -condebug).</span>
      ) : (
        <>
          <div className="flex items-start gap-2.5">
            <div
              className="mt-[7px] h-1.5 w-1.5 flex-none rounded-[3px]"
              style={{
                background: sum.color,
                boxShadow: `0 0 8px ${sum.color}`,
              }}
            />
            <div className="flex min-w-0 flex-col gap-0.5">
              <span data-testid="chat-state" style={{ font: fnt(500, 13), color: '#ece6dc' }}>
                {sum.value}
              </span>
              <span className="break-all" style={{ font: fnt(400, 10.5, 'mono', 1.45), color: '#6e6961' }}>
                {sum.detail}
              </span>
              {status && status.state === 'following' && <span style={{ font: fnt(400, 10.5, 'mono'), color: '#6e6961' }}>{chatCounts(status)}</span>}
            </div>
          </div>
          {sum.action && (
            <div
              data-testid="chat-action"
              className="rounded-xl px-3 py-2.5"
              style={{
                background: 'rgba(255,162,74,.07)',
                border: '1px solid rgba(255,162,74,.22)',
                font: fnt(400, 11.5, 'sans', 1.5),
                color: '#ffc68a',
              }}
            >
              {sum.action}
            </div>
          )}
          {status?.rulesWarning && <span style={{ font: fnt(400, 11, 'sans', 1.45), color: '#ff8a7a' }}>{status.rulesWarning}</span>}
          <div className="flex flex-col gap-1.5">
            <span className="lbl">DOSSIER DE CS2 (SI NON DÉTECTÉ)</span>
            <div className="flex gap-2">
              <input
                className="text-input min-w-0 flex-1"
                aria-label="Dossier de Counter-Strike 2"
                placeholder="…\steamapps\common\Counter-Strike Global Offensive"
                value={s.chatPath}
                onChange={(e) => s.set({ chatPath: e.target.value })}
              />
              <div className="pill flex-none px-3.5 py-[7px]" role="button" onClick={() => chat.rescan().then((st) => st && useChat.setState({ status: st }))} style={{ font: fnt(500, 11.5) }}>
                Rechercher
              </div>
            </div>
          </div>
        </>
      )}
      <div className="flex flex-col gap-1.5">
        <span className="lbl">VOTRE PSEUDO</span>
        <input className="text-input" aria-label="Votre pseudo dans CS2" placeholder="Pseudo Steam affiché dans le chat" value={s.ownName} onChange={(e) => s.set({ ownName: e.target.value })} />
      </div>
      <ToggleRow title="Ignorer mes messages" sub="Vos propres lignes de chat ne sont ni affichées ni traduites" on={s.tg.ignoreOwn} onClick={() => s.toggle('ignoreOwn')} />
    </div>
  );
}
