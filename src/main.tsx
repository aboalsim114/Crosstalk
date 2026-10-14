import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import App from './App';
import { diagnostics, isTauri } from './backend';
import { loadSettings, startSettingsSync } from './state/persist';
import { useApp } from './state/store';
import './styles/global.css';
import { prefillLanguages } from './state/langs';

// Interface errors go to the diagnostic journal (logs.rs) of the desktop app.
if (isTauri) {
  window.addEventListener('error', (e) => void diagnostics.write('ERROR', 'window', `${e.message} (${e.filename?.split('/').pop()}:${e.lineno})`));
  window.addEventListener('unhandledrejection', (e) => void diagnostics.write('ERROR', 'promise', String(e.reason)));
}

// The stored settings are loaded before the first render, so the engines start
// with the user's model, sources and glossary rather than the defaults.
loadSettings().then((warnings) => {
  // First launch: languages prefilled from Windows' (the user confirms them).
  const prefill = prefillLanguages(useApp.getState(), navigator.language);
  if (prefill) useApp.setState(prefill);
  createRoot(document.getElementById('root')!).render(
    <StrictMode>
      <App />
    </StrictMode>,
  );
  startSettingsSync();
  if (warnings.length) useApp.getState().flash(warnings[0]);
});
