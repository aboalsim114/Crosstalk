/// <reference types="vitest/config" />
import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import tailwindcss from '@tailwindcss/vite';
import pkg from './package.json' with { type: 'json' };

// The UI is embedded in the Tauri app: Vite only serves it during `tauri dev`
// (fixed port 1420) and bundles it into dist/, which Tauri packs into the .exe.
export default defineConfig({
  plugins: [react(), tailwindcss()],
  clearScreen: false,
  server: { port: 1420, strictPort: true },
  envPrefix: ['VITE_', 'TAURI_ENV_'],
  // The version shown on screen comes from package.json (checked equal to the
  // Rust and Tauri versions by scripts/validate.ps1 -Release).
  define: { __APP_VERSION__: JSON.stringify(pkg.version) },
  build: {
    // WebView2 is Chromium-based and evergreen.
    target: 'chrome120',
    outDir: 'dist',
    chunkSizeWarningLimit: 1200,
    // Three windows: the main app, the in-game overlay and the reply pop-up.
    rollupOptions: {
      input: { main: 'index.html', overlay: 'overlay.html', reply: 'reply.html' },
    },
  },
  test: {
    environment: 'jsdom',
    setupFiles: ['./src/test/setup.ts'],
    css: false,
  },
});
