// Captures the real Crosstalk interface (web build in demo mode, outside Tauri)
// at 2x density, to be used as textures by the showcase scenes.
// Prerequisite: `npx vite --port 5199` running at the repository root.
import { chromium } from 'playwright-core';
import { mkdirSync } from 'node:fs';

const BASE = process.env.APP_URL ?? 'http://localhost:5199';
const OUT = new URL('../public/screens/', import.meta.url).pathname;
mkdirSync(OUT, { recursive: true });
const executablePath = process.env.CHROMIUM ?? '/opt/pw-browsers/chromium-1194/chrome-linux/chrome';

const browser = await chromium.launch({ executablePath, args: ['--use-gl=angle', '--use-angle=swiftshader', '--enable-unsafe-swiftshader'] });
const ctx = await browser.newContext({ viewport: { width: 1384, height: 904 }, deviceScaleFactor: 2, locale: 'fr-FR', timezoneId: 'Europe/Paris' });
const page = await ctx.newPage();
const shot = async (name, sel = '.app-shell') => {
  await page.locator(sel).first().screenshot({ path: OUT + name + '.png' });
  console.log('screen', name);
};
const wait = (ms) => page.waitForTimeout(ms);

await page.goto(BASE + '/');
await wait(4000);
await shot('01-intro');
await page.getByText('Entrer et démarrer la session', { exact: true }).click();
// First launch asks for the languages: French reader, Russian and English teammates.
const dialog = page.locator('[role=dialog][aria-label="Vos langues"]');
await dialog.waitFor({ timeout: 8000 });
await wait(1200);
await shot('00-first-launch');
await dialog.getByText('Continuer', { exact: true }).click();
await wait(16000);
await shot('02-dashboard');
const nav = async (label, name, ms = 2500) => {
  await page.locator('.nav-item', { hasText: label }).first().click();
  await wait(ms);
  await shot(name);
};
await nav('Overlay', '03-overlay');
await nav('Audio', '04-audio');
await nav('Langues', '05-languages');
await nav('Historique', '06-history');
await nav('Diagnostic', '07-diagnostic');
await nav('Accueil', '08-dashboard-later', 8000);

// The in-game overlay window on its own (transparent window in the desktop app).
const ov = await ctx.newPage();
await ov.setViewportSize({ width: 760, height: 420 });
await ov.goto(BASE + '/overlay.html');
await ov.waitForTimeout(5000);
await ov.screenshot({ path: OUT + '09-overlay-window.png', omitBackground: true });
console.log('screen 09-overlay-window');
await browser.close();
