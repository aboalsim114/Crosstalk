import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import App from './App';
import { loadSettings, startSettingsSync } from './state/persist';
import { useApp } from './state/store';
import './styles/global.css';

// The stored settings are loaded before the first render, so the engines start
// with the user's model, sources and glossary rather than the defaults.
loadSettings().then((warnings) => {
  createRoot(document.getElementById('root')!).render(
    <StrictMode>
      <App />
    </StrictMode>,
  );
  startSettingsSync();
  if (warnings.length) useApp.getState().flash(warnings[0]);
});
