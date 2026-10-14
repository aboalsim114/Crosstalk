import { useEffect } from 'react';
import { motion } from 'motion/react';
import { isTauri } from './backend';
import { config } from './config';
import { startNativeBridge } from './native';
import { EASE } from './components/atoms';
import { Intro, Letterbox, Toast } from './components/Intro';
import { PageHeader } from './components/PageHeader';
import { SetupBanner } from './components/SetupBanner';
import { UpdateBanner } from './components/UpdateBanner';
import { startUpdateBridge } from './state/update';
import { startHistoryRecorder } from './state/history';
import { startGsiBridge } from './state/gsi';
import { startStreamBridge } from './state/stream';
import { startSpeakerBridge } from './state/speaker';
import { startRecapTracker } from './state/recap';
import { startLocaleBridge } from './i18n/bridge';
import { Sidebar } from './components/Sidebar';
import { TitleBar } from './components/TitleBar';
import { AboutPage } from './pages/AboutPage';
import { AudioPage } from './pages/AudioPage';
import { Dashboard } from './pages/Dashboard';
import { DiagPage } from './pages/DiagPage';
import { HistoryPage } from './pages/HistoryPage';
import { LangPage } from './pages/LangPage';
import { OverlayPage } from './pages/OverlayPage';
import { Stage } from './scene/Stage';
import type { PageId } from './state/data';
import { startMeter, useApp } from './state/store';

const PAGES: Record<PageId, () => React.JSX.Element> = {
  dash: Dashboard,
  overlay: OverlayPage,
  audio: AudioPage,
  lang: LangPage,
  hist: HistoryPage,
  diag: DiagPage,
  about: AboutPage,
};

/** Ctrl+Alt+F8 overlay, F9 pause, F10 emergency stop (focused window, outside Tauri). */
export function handleShortcut(e: Pick<KeyboardEvent, 'ctrlKey' | 'altKey' | 'key' | 'preventDefault'>) {
  if (!(e.ctrlKey && e.altKey)) return;
  const s = useApp.getState();
  const action = { F8: s.toggleOverlay, F9: s.togglePause, F10: s.emergency }[e.key];
  if (action) {
    e.preventDefault();
    action();
  }
}

export default function App() {
  const page = useApp((s) => s.page);
  const entered = useApp((s) => s.entered);
  const showIntro = useApp((s) => s.showIntro);
  const Page = PAGES[page];

  useEffect(() => {
    const stopMeter = startMeter();
    // In the desktop app the shortcuts are system-wide (registered in Rust) and arrive
    // through the native bridge; the window listener only serves the UI dev server.
    const stopNative = startNativeBridge();
    const stopUpdate = startUpdateBridge();
    const stopHistory = isTauri ? startHistoryRecorder() : () => {};
    const stopGsi = startGsiBridge();
    const stopStream = isTauri ? startStreamBridge() : () => {};
    const stopSpeaker = startSpeakerBridge();
    const stopLocale = startLocaleBridge();
    const stopRecap = startRecapTracker((m) => useApp.getState().flash(`Résumé de ${m.mapName ?? 'la partie'} prêt · onglet Historique`));
    if (!isTauri) window.addEventListener('keydown', handleShortcut);
    let autoStart: ReturnType<typeof setTimeout> | undefined;
    if (config.autoStart) autoStart = setTimeout(() => useApp.getState().toggleSession(), 400);
    return () => {
      stopMeter();
      stopNative();
      stopUpdate();
      stopHistory();
      stopGsi();
      stopStream();
      stopSpeaker();
      stopLocale();
      stopRecap();
      window.removeEventListener('keydown', handleShortcut);
      clearTimeout(autoStart);
    };
  }, []);

  // Outside the desktop shell (UI-only dev server) show the window as a framed mock.
  useEffect(() => {
    document.body.classList.toggle('framed', !isTauri);
  }, []);

  const shell = (
    <div className={`app-shell ${isTauri ? 'native' : 'framed'}`} data-screen-label="App window">
      <TitleBar />
      <div className="relative min-h-0 flex-1">
        <Stage />
        <div className="pointer-events-none absolute inset-0 z-[1]" style={{ background: 'radial-gradient(ellipse 75% 70% at 45% 42%,transparent 40%,rgba(0,0,0,.7) 100%)' }} />

        <div
          className="absolute inset-0 z-[2] flex"
          style={{
            opacity: entered ? 1 : 0,
            transform: `translateY(${entered ? 0 : 14}px)`,
            transition: 'opacity 1.1s ease .35s,transform 1.4s cubic-bezier(.2,.7,.1,1) .35s',
          }}
          aria-hidden={!entered}
        >
          <Sidebar />
          <main className="flex min-w-0 flex-1 flex-col">
            <PageHeader />
            <UpdateBanner />
            <SetupBanner />
            <motion.div
              key={page}
              initial={{ opacity: 0, y: 14, filter: 'blur(8px)' }}
              // Drop the filter once settled: any filter on an ancestor would stop the
              // glass panels' backdrop-filter from blurring the 3D stage behind them.
              animate={{ opacity: 1, y: 0, filter: 'blur(0px)', transitionEnd: { filter: 'none' } }}
              transition={{ duration: 0.75, ease: EASE }}
              className="min-h-0 flex-1 overflow-auto px-8 pt-1 pb-[26px]"
            >
              <Page />
            </motion.div>
          </main>
        </div>

        {showIntro && <Intro />}
        <Letterbox />
      </div>
      <div className="grain" />
      <Toast />
    </div>
  );

  return isTauri ? shell : <div className="frame-host">{shell}</div>;
}
