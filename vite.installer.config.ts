import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';

// The installer's UI (installer.html, "Crosstalk Installer" design), bundled
// apart from the app into dist-installer/ and embedded in src-installer.
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: { port: 1430, strictPort: true },
  build: {
    target: 'chrome120',
    outDir: 'dist-installer',
    chunkSizeWarningLimit: 1200,
    rollupOptions: { input: { installer: 'installer.html' } },
  },
});
