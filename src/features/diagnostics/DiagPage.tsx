import { useEffect, useState } from 'react';
import { chat, diagnostics, getShortcutStatus, type LogLine, type PulseView, type ShortcutStatus } from '../../shared/ipc';
import { config } from '../../app/config';
import { fnt } from '../../shared/ui/atoms';
import { useAudio, type AudioState } from '../audio/audio';
import { engineSummary, useSpeech } from '../speech/speech';
import { percentile, translationHealth, translationSummary, useTranslation, type TranslationHealth } from '../translation/translation';
import { useApp } from '../../app/state/store';
import { useChat } from '../chat/chat';
import type { ChatStatus } from '../../shared/ipc';

type DiagState = 'OK' | 'AVERT.' | 'ÉCHEC';

/** Diagnostic verdict of the translation, from its state (never from a display colour). */
export function translationDiag(health: TranslationHealth): DiagState {
  if (health === 'ready') return 'OK';
  return 'AVERT.';
}

/** Real process-loopback row from the Rust capture and the Windows build. */
export function captureDiag(a: Pick<AudioState, 'platform' | 'status'>): [string, DiagState] {
  const build = a.platform?.windowsBuild;
  const doc = a.platform?.processLoopbackDocumented;
  const base = build ? `Build ${build}${doc === false ? ' < 20348 (non documenté, tenté)' : ''}` : 'Build inconnu';
  const st = a.status;
  if (st.state === 'error') return [`${base} · ${st.error?.message ?? 'erreur'}`, 'ÉCHEC'];
  if (st.state === 'capturing' && st.source?.kind === 'process') return [`${base} · ${st.source.processName} ciblé (PID ${st.source.pid})`, 'OK'];
  if (st.state === 'capturing') return [`${base} · repli système${st.fallbackReason ? ` — ${st.fallbackReason}` : ''}`, 'AVERT.'];
  return [`${base} · non testé dans cette session`, doc === false ? 'AVERT.' : 'OK'];
}

/** Real console.log, -condebug and parser rows from the Rust chat service. */
export function chatDiag(st: ChatStatus | null): [string, string, DiagState][] {
  if (!st) return [['Journal console.log', 'Recherche de CS2…', 'AVERT.']];
  const s = st.setup;
  const log: [string, string, DiagState] = !s.consoleLog
    ? ['Journal console.log', s.steamDir ? 'CS2 introuvable dans les bibliothèques Steam : indiquez son dossier (Langues & glossaire)' : 'Steam introuvable : indiquez le dossier de CS2', 'ÉCHEC']
    : [`Journal console.log${s.manual ? ' (chemin manuel)' : ''}`, s.consoleLog + (s.consoleLogExists ? '' : ' · pas encore créé (lancez CS2 avec -condebug)'), s.consoleLogExists ? 'OK' : 'AVERT.'];
  const condebug: [string, string, DiagState] =
    s.condebug === 'present'
      ? ['Option -condebug', 'Présente dans les options de lancement Steam', 'OK']
      : s.condebug === 'absent'
        ? ['Option -condebug', 'Absente : Steam › CS2 › Propriétés › Options de lancement', 'ÉCHEC']
        : ['Option -condebug', 'Options de lancement Steam illisibles : à vérifier à la main', 'AVERT.'];
  const unknown = st.unrecognized ? ` · ${st.unrecognized} ligne${st.unrecognized > 1 ? 's' : ''} de chat non reconnue${st.unrecognized > 1 ? 's' : ''} (forme dans le journal)` : '';
  const rules: [string, string, DiagState] = [
    'Parseur de chat',
    `Règles ${st.rulesVersion} · ${st.rulesFile ?? 'intégrées'}${st.rulesWarning ? ` · ${st.rulesWarning}` : ''}${unknown}`,
    st.rulesWarning || st.unrecognized ? 'AVERT.' : 'OK',
  ];
  return [log, condebug, rules];
}

/** What the Rust services report now (no fabricated history). */
export function nativeLog(chat: ChatStatus | null, lines: [string, string][]): [string, string, string][] {
  const out: [string, string, string][] = [];
  if (chat) {
    out.push(['', 'INFO', `cs2-detect: ${chat.setup.libraries.length} bibliothèque(s) Steam${chat.setup.cs2Dir ? ' · CS2 trouvé' : ' · CS2 introuvable'}`]);
    if (chat.state === 'following') out.push(['', 'INFO', `chat-ingest: console.log suivi, ${chat.offset.toLocaleString('fr-FR')} octets lus, ${chat.messages} message(s)`]);
    if (chat.state === 'waiting') out.push(['', 'WARN', 'chat-ingest: console.log absent, en attente du lancement de CS2']);
    if (chat.rulesWarning) out.push(['', 'WARN', `chat-rules: ${chat.rulesWarning}`]);
  }
  for (const [lv, m] of lines) out.push(['', lv, m]);
  return out;
}

/** "Voix → texte" row: the stage that stops during this capture, else the last 30 s summary. */
export function pulseDiag(p: PulseView | null): [string, DiagState] {
  if (!p || (!p.capturing && !p.summary)) return ['Démarrez une session : bilan toutes les 30 s (son, VAD, reconnaissance, traduction)', 'AVERT.'];
  const last = p.alerts[p.alerts.length - 1];
  if (last) return [last, 'ÉCHEC'];
  return [p.summary ?? 'Capture en cours : premier bilan dans 30 s', 'OK'];
}

/** Journal lines as shown on screen (local time). */
export function journalRows(lines: LogLine[]): [string, string, string][] {
  return lines.map((l) => [new Date(l.ts).toLocaleTimeString('fr-FR'), l.level, `${l.source}: ${l.message}`]);
}

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
  const [journal, setJournal] = useState<LogLine[]>([]);
  const [pulse, setPulse] = useState<PulseView | null>(null);
  const [exported, setExported] = useState<string | null>(null);
  const audio = useAudio();
  const [captureDetail, captureState] = audio.native
    ? captureDiag(audio)
    : [fb ? 'Build 19045 < 20348 — repli système' : 'Build 26100 · cs2.exe ciblé', (fb ? 'AVERT.' : 'OK') as DiagState];
  const sp = useSpeech();
  const engine = engineSummary(sp.status);
  const asrRow: [string, string, DiagState] = sp.native
    ? ['Reconnaissance vocale', `${engine.value} · ${engine.detail}`, sp.status?.state === 'ready' ? 'OK' : sp.status?.state === 'error' ? 'ÉCHEC' : 'AVERT.']
    : ['Reconnaissance vocale', 'Parakeet TDT 0.6B v3 · chargé en 6,1 s · CPU', 'OK'];
  const trStatus = useTranslation((x) => x.status);
  const trTimes = useTranslation((x) => x.times);
  const trSum = translationSummary(trStatus);
  const p50 = percentile(trTimes, 0.5);
  const trRow: [string, string, DiagState] = sp.native
    ? ['Traduction IA · LLM', `${trSum.value}${p50 != null ? ` · p50 ${p50} ms` : ''}`, translationDiag(translationHealth(trStatus))]
    : ['Traduction IA · LLM', down ? 'Délai dépassé (8 s) — repli sur l’original' : 'Réponse en 210 ms', down ? 'ÉCHEC' : 'OK'];
  const chatStatus = useChat((x) => x.status);
  const chatRows: [string, string, DiagState][] = sp.native
    ? chatDiag(chatStatus)
    : [
        ['Journal console.log', 'D:\\SteamLibrary\\steamapps\\common\\Counter-Strike Global Offensive\\game\\csgo\\console.log', 'OK'],
        ['Option -condebug', 'Détectée dans les options Steam', 'OK'],
        ['Parseur de chat', 'cs2-2026.09 · 42 fixtures validées', 'OK'],
      ];
  const rows: [string, string, DiagState][] = [
    ...chatRows,
    ['Process loopback', captureDetail, captureState],
    asrRow,
    ...(sp.native ? [['Voix → texte', ...pulseDiag(pulse)] as [string, string, DiagState]] : []),
    trRow,
    keysFailed.length
      ? ['Raccourcis globaux', `${keysFailed.map((k) => k.keys).join(', ')} déjà utilisé par une autre application`, 'AVERT.']
      : ['Raccourcis globaux', 'Mouse 4 / Mouse 5 / Ctrl+Alt+F6 à F10 · aucun conflit', 'OK'],
  ];
  const demoLogs: [string, string, string][] = [
    ['14:02:11', 'INFO', 'app-shell: instance unique acquise'],
    ['14:02:11', 'INFO', 'cs2-detect: 2 bibliothèques Steam'],
    ['14:02:12', 'INFO', 'chat-ingest: suivi depuis offset 1 284 330'],
    ['14:02:12', fb ? 'WARN' : 'INFO', fb ? 'audio-win: repli endpoint loopback' : 'audio-win: process loopback pid=18244'],
    ['14:02:13', 'INFO', 'asr: Parakeet TDT 0.6B v3 chargé (CPU)'],
    ['14:02:13', 'INFO', 'traduction : LLM prêt → fr'],
    ['14:03:40', 'DEBUG', 'audio-pipeline: segment 3,1 s → asr'],
    ['14:03:41', 'WARN', 'audio-pipeline: segment périmé abandonné'],
    ['14:03:44', 'INFO', 'overlay: 2 blocs affichés'],
  ];
  useEffect(() => {
    if (!sp.native) return;
    const load = () => {
      diagnostics.recent(80).then(setJournal);
      diagnostics.pulse().then(setPulse);
    };
    load();
    const t = setInterval(load, 2000);
    return () => clearInterval(t);
  }, [sp.native]);
  // In the desktop app the tests are re-run for real: the chat set-up is
  // scanned again and the shortcut registrations re-read.
  const run = !sp.native
    ? s.runDiag
    : async () => {
        useApp.setState({ diag: 'running' });
        const [st] = await Promise.all([chat.rescan(), getShortcutStatus().then(setKeys), new Promise((r) => setTimeout(r, 400))]);
        if (st) useChat.setState({ status: st });
        useApp.setState({ diag: 'done' });
      };
  const report = () =>
    diagnostics.report(rows.map(([name, detail, state]) => ({ name, detail, state }))).then(
      (path) => {
        setExported(path);
        s.flash(`Rapport exporté : ${path}`);
      },
      (e) => s.flash(`Export impossible : ${e}`),
    );
  // Newest first; before the first journal line, what the services report now.
  const logs = journal.length
    ? journalRows(journal).reverse()
    : sp.native
      ? nativeLog(chatStatus, [
          [captureState === 'ÉCHEC' ? 'WARN' : 'INFO', `audio: ${captureDetail}`],
          [sp.status?.state === 'ready' ? 'INFO' : 'WARN', `asr: ${engine.value} · ${engine.detail}`],
          ['INFO', `translation: ${trSum.value}`],
        ])
      : demoLogs;
  return (
    <div className="grid grid-cols-2 items-start gap-4">
      <div className="glass flex flex-col overflow-hidden">
        <div className="flex items-center justify-between px-5 py-3.5" style={{ borderBottom: '1px solid rgba(255,255,255,.05)' }}>
          <span style={{ font: fnt(400, 22, 'serif'), color: '#ece6dc' }}>Tests intégrés</span>
          <div className="pill pill-ember px-4 py-[7px]" role="button" onClick={run} style={{ font: fnt(500, 12) }}>
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
      <div className="flex flex-col gap-4">
      <div className="flex max-h-[520px] flex-col gap-[5px] overflow-y-auto rounded-2xl px-[18px] py-4" style={{ background: 'rgba(0,0,0,.55)', border: '1px solid rgba(255,255,255,.06)', font: fnt(400, 11, 'mono', 1.55), color: '#8f8a83' }}>
        <div className="flex justify-between pb-1.5" style={{ color: '#5a5650', letterSpacing: '.14em', fontSize: 9.5 }}>
          <span>JOURNAL · EXPURGÉ</span>
          <span>{sp.native ? 'LOGS · 7 JOURS' : 'DÉMO'}</span>
        </div>
        {logs.map(([t, lv, m], i) => (
          <div key={i}>
            <span style={{ color: '#4a4640' }}>{t}</span> <span style={{ color: lv === 'ERROR' ? '#ff8a7a' : lv === 'WARN' ? '#ffa24a' : lv === 'DEBUG' ? '#4a4640' : '#7fe0a8' }}>{lv}</span> {m}
          </div>
        ))}
      </div>
      {sp.native && (
        <div className="glass flex flex-col gap-3 p-5">
          <span style={{ font: fnt(400, 12, 'sans', 1.5), color: '#6e6961' }}>
            Un problème ? Exportez le rapport (versions, tests, journal ; ni audio, ni texte de partie, ni pseudo) puis joignez-le à votre signalement.
          </span>
          <div className="flex flex-wrap gap-2">
            <div className="pill pill-ember px-4 py-[7px]" role="button" onClick={report} style={{ font: fnt(500, 12) }}>
              Exporter le rapport
            </div>
            <div className="pill px-4 py-[7px]" role="button" onClick={() => diagnostics.reportIssue(!!exported).catch((e) => s.flash(String(e)))} style={{ font: fnt(500, 12) }}>
              Signaler un problème
            </div>
            <div className="pill px-4 py-[7px]" role="button" onClick={() => diagnostics.openFolder().catch((e) => s.flash(String(e)))} style={{ font: fnt(500, 12) }}>
              Dossier des journaux
            </div>
          </div>
          {exported && <span className="truncate" style={{ font: fnt(400, 10.5, 'mono'), color: '#5a5650' }}>{exported}</span>}
        </div>
      )}
      </div>
    </div>
  );
}
