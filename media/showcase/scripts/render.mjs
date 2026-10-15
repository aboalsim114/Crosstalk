// Renders the film frame by frame in headless Chromium and pipes PNG frames to FFmpeg.
//   node scripts/render.mjs                    → out/video.mp4 (no sound, 1920×1080, 30 i/s)
//   node scripts/render.mjs --stills 3,12.5    → out/stills/t3.00.png …
//   node scripts/render.mjs --from 12 --to 22  → a partial render (out/part-12-22.mp4)
// The page is served by Vite (started here) from this folder.
import { chromium } from 'playwright-core';
import { spawn } from 'node:child_process';
import { mkdirSync } from 'node:fs';
import { createServer } from 'vite';

const args = process.argv.slice(2);
const opt = (name, def) => {
  const i = args.indexOf(`--${name}`);
  return i >= 0 ? args[i + 1] : def;
};
const root = new URL('..', import.meta.url).pathname;
const out = root + 'out/';
mkdirSync(out + 'stills', { recursive: true });

const server = await createServer({ root, logLevel: 'error', server: { port: 5300, strictPort: false, hmr: false, watch: null } });
await server.listen();
const url = `http://localhost:${server.config.server.port}/?render=1`;

const executablePath = process.env.CHROMIUM ?? '/opt/pw-browsers/chromium-1194/chrome-linux/chrome';
const browser = await chromium.launch({
  executablePath,
  args: ['--use-gl=angle', '--use-angle=swiftshader', '--enable-unsafe-swiftshader', '--ignore-gpu-blocklist', '--disable-background-timer-throttling', '--force-color-profile=srgb', '--font-render-hinting=none'],
});
const page = await browser.newPage({ viewport: { width: 1920, height: 1080 }, deviceScaleFactor: 1 });
page.on('console', (m) => m.type() === 'error' && console.error('[page]', m.text()));
page.on('pageerror', (e) => console.error('[page]', e.message));
await page.goto(url);
await page.waitForFunction(() => window.__ready || window.__error, null, { timeout: 120000 });
const err = await page.evaluate(() => window.__error);
if (err) throw new Error(err);
const meta = await page.evaluate(() => window.__meta);
const stage = page.locator('#stage');

const stills = opt('stills');
if (args.includes('--cover')) {
  // Cover for the README: the finale composition, the wordmark and a play badge.
  await page.evaluate(() => window.__hideDust());
  await page.evaluate((x) => window.__seek(x), 44.6);
  await page.evaluate(() => {
    const dom = document.getElementById('dom');
    const layer = document.createElement('div');
    layer.className = 'layer';
    layer.innerHTML = `
      <div style="position:absolute;left:0;right:0;top:96px;text-align:center;font:400 150px/1 'Instrument Serif';letter-spacing:-.02em;color:#f3ede3;text-shadow:0 6px 60px rgba(0,0,0,.9)">Cross<span style="font-style:italic;color:#ffa24a">talk</span></div>
      <div style="position:absolute;left:50%;top:268px;width:560px;height:2px;margin-left:-280px;background:linear-gradient(90deg,#ffa24a,#ffc68a 50%,rgba(127,212,255,0))"></div>
      <div style="position:absolute;left:0;right:0;top:296px;text-align:center;font:400 44px/1.1 'Instrument Serif';color:#ece6dc">Comprenez le call. <i style="color:#ffc68a">Restez dans le jeu.</i></div>
      <div style="position:absolute;left:50%;top:520px;width:132px;height:132px;margin:-66px 0 0 -66px;border-radius:50%;background:rgba(6,6,8,.62);border:2px solid rgba(255,162,74,.9);box-shadow:0 0 60px rgba(255,162,74,.45),inset 0 0 30px rgba(255,162,74,.15);backdrop-filter:blur(6px)"></div>
      <div style="position:absolute;left:50%;top:520px;width:0;height:0;margin:-24px 0 0 -14px;border-left:44px solid #ffc68a;border-top:24px solid transparent;border-bottom:24px solid transparent"></div>
      <div style="position:absolute;left:0;right:0;top:612px;text-align:center;font:500 18px 'JetBrains Mono';letter-spacing:.38em;color:#ffc68a;text-shadow:0 2px 16px #000">VOIR LA VIDÉO · 50 S</div>
      <div style="position:absolute;left:0;right:0;bottom:64px;text-align:center;font:500 16px 'JetBrains Mono';letter-spacing:.34em;color:#6e6961">WINDOWS 10 / 11 · TRADUCTION LOCALE · COUNTER-STRIKE 2</div>`;
    dom.appendChild(layer);
  });
  await stage.screenshot({ path: `${out}cover.png` });
  console.log('cover', `${out}cover.png`);
} else if (stills) {
  for (const s of stills.split(',')) {
    const t = parseFloat(s);
    await page.evaluate((x) => window.__seek(x), t);
    await stage.screenshot({ path: `${out}stills/t${t.toFixed(2)}.png` });
    console.log('still', t);
  }
} else {
  const from = parseFloat(opt('from', '0'));
  const to = parseFloat(opt('to', String(meta.DURATION)));
  const file = opt('out', from === 0 && to === meta.DURATION ? `${out}video.mp4` : `${out}part-${from}-${to}.mp4`);
  const fps = meta.FPS;
  const ff = spawn('ffmpeg', ['-y', '-loglevel', 'error', '-f', 'image2pipe', '-framerate', String(fps), '-c:v', 'png', '-i', '-',
    '-c:v', 'libx264', '-preset', 'slow', '-crf', '12', '-pix_fmt', 'yuv420p', '-colorspace', 'bt709', '-color_primaries', 'bt709', '-color_trc', 'bt709', file], { stdio: ['pipe', 'inherit', 'inherit'] });
  const n0 = Math.round(from * fps), n1 = Math.round(to * fps);
  const t0 = Date.now();
  for (let n = n0; n < n1; n++) {
    await page.evaluate((x) => window.__seek(x), n / fps);
    const buf = await stage.screenshot({ type: 'png' });
    if (!ff.stdin.write(buf)) await new Promise((r) => ff.stdin.once('drain', r));
    if ((n - n0) % 30 === 0) {
      const el = (Date.now() - t0) / 1000;
      const done = n - n0 + 1;
      console.log(`frame ${n}/${n1} · ${(el / done).toFixed(2)} s/frame · ETA ${(((n1 - n) * el) / done / 60).toFixed(1)} min`);
    }
  }
  ff.stdin.end();
  await new Promise((r) => ff.on('close', r));
  console.log('written', file);
}
await browser.close();
await server.close();
