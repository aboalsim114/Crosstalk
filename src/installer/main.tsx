import '@fontsource-variable/archivo/wdth.css';
import '@fontsource/geist-sans/400.css';
import '@fontsource/geist-sans/500.css';
import '@fontsource/jetbrains-mono/400.css';
import '@fontsource/jetbrains-mono/500.css';
import '@fontsource/jetbrains-mono/600.css';
import { createRoot } from 'react-dom/client';
import { Installer } from './Installer';

// No StrictMode: its double effects would build the WebGL stage twice.
// ?screen=N opens a given screen directly (design review, screenshots).
const screen = Number(new URLSearchParams(location.search).get('screen') ?? 0);

createRoot(document.getElementById('root')!).render(
  <Installer startScreen={Math.min(3, Math.max(0, screen || 0))} />,
);
