import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import { OverlayWindow } from './OverlayWindow';
import '../../styles/global.css';

document.documentElement.classList.add('overlay-window');

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <OverlayWindow />
  </StrictMode>,
);
