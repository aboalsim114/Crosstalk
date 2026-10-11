import { useEffect, useState } from 'react';
import { getShortcutStatus, type ShortcutStatus } from '../backend';
import { config } from '../config';
import { fnt } from '../components/atoms';
import { useApp } from '../state/store';

type DiagState = 'OK' | 'AVERT.' | 'ÉCHEC';

export function DiagPage() {
  const s = useApp();
  const fb = config.win10Fallback;
  const down = config.translatorDown;
  const done = s.diag === 'done';
  // Real registration result from the Rust core in the desktop app; simulated otherwise.
  const [keys, setKeys] = useState<ShortcutStatus[] | null>(null);
  useEffect(() => {
    getShortcutStatus().then(setKeys);
  }, [s.diag]);
  const keysFailed = keys?.filter((k) => !k.registered) ?? [];
  const rows: [string, string, DiagState][] = [
    ['Journal console.log', 'D:\\SteamLibrary\\steamapps\\common\\Counter-Strike Global Offensive\\game\\csgo\\console.log', 'OK'],
    ['Option -condebug', 'Détectée dans les options Steam', 'OK'],
    ['Parseur de chat', 'cs2-2026.09 · 42 fixtures validées', 'OK'],
    ['Process loopback', fb ? 'Build 19045 < 20348 — repli système' : 'Build 26100 · cs2.exe ciblé', fb ? 'AVERT.' : 'OK'],
    ['Modèle whisper-' + s.model, 'Chargé en 1,8 s · ' + s.accel.toUpperCase(), 'OK'],
    ['Traduction · ' + s.engine, down ? 'Délai dépassé (5 s) — repli sur l’original' : 'Réponse en 210 ms', down ? 'ÉCHEC' : 'OK'],
    keysFailed.length
      ? ['Raccourcis globaux', `${keysFailed.map((k) => k.keys).join(', ')} déjà utilisé par une autre application`, 'AVERT.']
      : ['Raccourcis globaux', 'Ctrl+Alt+F8 / F9 / F10 · aucun conflit', 'OK'],
  ];
  const logs: [string, string, string][] = [
    ['14:02:11', 'INFO', 'app-shell: instance unique acquise'],
    ['14:02:11', 'INFO', 'cs2-detect: 2 bibliothèques Steam'],
    ['14:02:12', 'INFO', 'chat-ingest: suivi depuis offset 1 284 330'],
    ['14:02:12', fb ? 'WARN' : 'INFO', fb ? 'audio-win: repli endpoint loopback' : 'audio-win: process loopback pid=18244'],
    ['14:02:13', 'INFO', 'asr: whisper-small chargé (CUDA)'],
    ['14:02:13', 'INFO', 'translation: DeepL key=[expurgée]'],
    ['14:03:40', 'DEBUG', 'audio-pipeline: segment 3,1 s → asr'],
    ['14:03:41', 'WARN', 'audio-pipeline: segment périmé abandonné'],
    ['14:03:44', 'INFO', 'overlay: 2 blocs affichés'],
  ];
  return (
    <div className="grid grid-cols-2 items-start gap-4">
      <div className="glass flex flex-col overflow-hidden">
        <div className="flex items-center justify-between px-5 py-3.5" style={{ borderBottom: '1px solid rgba(255,255,255,.05)' }}>
          <span style={{ font: fnt(400, 22, 'serif'), color: '#ece6dc' }}>Tests intégrés</span>
          <div className="pill pill-ember px-4 py-[7px]" role="button" onClick={s.runDiag} style={{ font: fnt(500, 12) }}>
            {done ? 'Relancer' : 'Tests…'}
          </div>
        </div>
        {rows.map(([name, detail, st]) => {
          const c = !done ? '#5a5650' : st === 'OK' ? '#7fe0a8' : st === 'AVERT.' ? '#ffa24a' : '#ff8a7a';
          return (
            <div key={name} className="grid items-center gap-3.5 px-5 py-[11px]" style={{ gridTemplateColumns: '10px minmax(0,1fr) auto', borderBottom: '1px solid rgba(255,255,255,.035)' }}>
              <div className="h-1.5 w-1.5 rounded-[3px]" style={{ background: c, boxShadow: `0 0 8px ${c}` }} />
              <div className="flex min-w-0 flex-col gap-0.5">
                <span style={{ font: fnt(500, 12.5), color: '#ece6dc' }}>{name}</span>
                <span className="truncate" style={{ font: fnt(400, 10.5, 'mono'), color: '#6e6961' }}>
                  {detail}
                </span>
              </div>
              <span style={{ font: fnt(600, 9.5, 'mono'), letterSpacing: '.12em', color: c }}>{done ? st : '…'}</span>
            </div>
          );
        })}
      </div>
      <div className="flex flex-col gap-[5px] rounded-2xl px-[18px] py-4" style={{ background: 'rgba(0,0,0,.55)', border: '1px solid rgba(255,255,255,.06)', font: fnt(400, 11, 'mono', 1.55), color: '#8f8a83' }}>
        <div className="flex justify-between pb-1.5" style={{ color: '#5a5650', letterSpacing: '.14em', fontSize: 9.5 }}>
          <span>JOURNAL · EXPURGÉ</span>
          <span>TRACING</span>
        </div>
        {logs.map(([t, lv, m], i) => (
          <div key={i}>
            <span style={{ color: '#4a4640' }}>{t}</span> <span style={{ color: lv === 'WARN' ? '#ffa24a' : lv === 'DEBUG' ? '#4a4640' : '#7fe0a8' }}>{lv}</span> {m}
          </div>
        ))}
      </div>
    </div>
  );
}
