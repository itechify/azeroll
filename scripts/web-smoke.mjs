#!/usr/bin/env node
// web-smoke.mjs - drive Chromium over raw CDP against a running wenilla-host: load the page, wait
// for the world, hold W, and write screenshots, the console and an fps estimate to OUT.
//
//   node scripts/web-smoke.mjs                       # native Wayland, http://127.0.0.1:8090/
//   OZONE=x11 node scripts/web-smoke.mjs             # XWayland
//   OUT=/tmp/azeroll-web URL=http://127.0.0.1:8090/ WAIT=240 node scripts/web-smoke.mjs
//
// Env: CHROME (default /usr/lib/chromium/chromium: the omarchy `chromium` wrapper appends
// --ozone-platform=wayland, which wins over ours), OZONE (x11|wayland|headless), FLAGS (replaces
// the GPU flags), PORT (CDP, default 9333), PROFILE (default /tmp/azeroll-chrome, wiped each run).
// KEEP=1 leaves chromium running afterwards (CDP stays on PORT) for poking at it.
// Needs Node >= 22 (global WebSocket and fetch). No dependencies.
//
// The GPU flags are what this NVIDIA + Hyprland box needed: without the Vulkan/ANGLE trio the WebGPU
// canvas stays black ("Error creating wgpu::Texture"), and without --disable-gpu-vsync
// --disable-frame-rate-limit Chromium delivers ~1 rAF/s to ANY WebGPU page, even a bare clear
// loop (no vblank feedback), so the game looks frozen and fps numbers mean nothing.
//
// Headless chromium on this NVIDIA box loads the world but cannot present the WebGPU canvas
// (blank page, "Error creating wgpu::Texture"), so the default is a real window. Under Hyprland
// the window is floated and parked so it does not reshuffle the tiling layout.
import { spawn, execSync } from 'node:child_process';
import fs from 'node:fs';

const OUT = process.env.OUT || '/tmp/azeroll-web';
const URL_ = process.env.URL || 'http://127.0.0.1:8090/';
const OZONE = process.env.OZONE || 'wayland';
const PORT = +(process.env.PORT || 9333);
const PROFILE = process.env.PROFILE || '/tmp/azeroll-chrome';
const WAIT_MS = +(process.env.WAIT || 240) * 1000;
const GPU_FLAGS =
  process.env.FLAGS ??
  '--enable-features=Vulkan,VulkanFromANGLE,DefaultANGLEVulkan --use-angle=vulkan --disable-gpu-vsync --disable-frame-rate-limit';
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const sh = (cmd) => { try { return execSync(cmd, { encoding: 'utf8' }); } catch { return ''; } };

fs.mkdirSync(OUT, { recursive: true });
fs.rmSync(PROFILE, { recursive: true, force: true });
const flags = [
  `--user-data-dir=${PROFILE}`, `--remote-debugging-port=${PORT}`, '--remote-debugging-address=127.0.0.1',
  '--enable-unsafe-webgpu', ...GPU_FLAGS.split(' ').filter(Boolean),
  '--no-first-run', '--no-default-browser-check', '--window-size=960,540',
  '--autoplay-policy=no-user-gesture-required',
  OZONE === 'headless' ? '--headless=new' : `--ozone-platform=${OZONE}`,
];
const known = new Set(JSON.parse(sh('hyprctl clients -j') || '[]').map((c) => c.address));
const chrome = spawn(process.env.CHROME || '/usr/lib/chromium/chromium', [...flags, 'about:blank'], {
  stdio: ['ignore', 'ignore', fs.openSync(`${OUT}/chrome-stderr.log`, 'w')],
});
const done = (code) => { if (!process.env.KEEP) try { chrome.kill(); } catch {} process.exit(code); };

let page;
for (let i = 0; i < 60 && !page; i++) {
  try { page = (await (await fetch(`http://127.0.0.1:${PORT}/json`)).json()).find((t) => t.type === 'page'); } catch {}
  if (!page) await sleep(300);
}
if (!page) { console.log('chromium never exposed a page'); done(1); }
if (OZONE !== 'headless') {
  for (let i = 0; i < 20; i++) { // float + park the window (Hyprland only; a no-op elsewhere)
    const c = JSON.parse(sh('hyprctl clients -j') || '[]').find((c) => !known.has(c.address) && /Chromium/.test(c.title));
    if (c) {
      const a = `address:${c.address}`;
      sh(`hyprctl dispatch setfloating ${a}`);
      sh(`hyprctl dispatch resizewindowpixel "exact 960 540,${a}"`);
      sh(`hyprctl dispatch movewindowpixel "exact 560 300,${a}"`);
      break;
    }
    await sleep(500);
  }
}

const ws = new WebSocket(page.webSocketDebuggerUrl);
await new Promise((r) => (ws.onopen = r));
let id = 0; const pending = new Map(); const lines = [];
ws.onmessage = (m) => {
  const d = JSON.parse(m.data);
  if (d.id && pending.has(d.id)) { pending.get(d.id)(d); pending.delete(d.id); }
  else if (d.method === 'Runtime.consoleAPICalled')
    lines.push(`[${d.params.type}] ` + d.params.args.map((a) => a.value ?? a.description ?? '').join(' '));
  else if (d.method === 'Runtime.exceptionThrown') lines.push('[exception] ' + JSON.stringify(d.params.exceptionDetails).slice(0, 2000));
  else if (d.method === 'Log.entryAdded') lines.push(`[log:${d.params.entry.level}] ${d.params.entry.text} ${d.params.entry.url || ''}`);
};
const send = (method, params = {}) => new Promise((res) => { const i = ++id; pending.set(i, res); ws.send(JSON.stringify({ id: i, method, params })); });
const ev = async (expression) => (await send('Runtime.evaluate', { expression, awaitPromise: true, returnByValue: true })).result?.result?.value;
const shot = async (name) => { fs.writeFileSync(`${OUT}/${name}.png`, Buffer.from((await send('Page.captureScreenshot', { format: 'png' })).result.data, 'base64')); console.log('shot', name); };
const key = (type, k, code, vk) => send('Input.dispatchKeyEvent', { type, key: k, code, windowsVirtualKeyCode: vk, nativeVirtualKeyCode: vk });
// rAF frames per second over `ms`, from a counter installed before the page's own scripts.
const fps = async (ms) => { const a = await ev('window.__f'); await sleep(ms); return ((await ev('window.__f')) - a) / (ms / 1000); };

await send('Runtime.enable'); await send('Log.enable'); await send('Page.enable');
// navigator.gpu exists only in a secure context (127.0.0.1 counts, about:blank does not).
await send('Page.navigate', { url: new URL('/favicon.svg', URL_).href }); await sleep(1500);
const gpu = await ev(`(async()=>{ if(!navigator.gpu) return 'no navigator.gpu'; const a = await navigator.gpu.requestAdapter(); if(!a) return 'no adapter'; const i = a.info; return JSON.stringify({vendor:i.vendor,arch:i.architecture,fallback:a.isFallbackAdapter}); })()`);
console.log('WebGPU adapter:', gpu); fs.writeFileSync(`${OUT}/gpu.txt`, String(gpu));
if (!String(gpu).startsWith('{')) { console.log('no WebGPU adapter: the world cannot run'); done(2); }

await send('Page.addScriptToEvaluateOnNewDocument', { source: 'window.__f=0;(function f(){window.__f++;requestAnimationFrame(f)})();' });
await send('Page.navigate', { url: URL_ });
const t0 = Date.now(); let ready = false;
while (Date.now() - t0 < WAIT_MS) {
  await sleep(5000);
  const secs = Math.round((Date.now() - t0) / 1000);
  console.log(`${secs}s overlay=${JSON.stringify(((await ev(`(document.getElementById('wenilla-boot')||{}).innerText||'gone'`)) || '').split('\n').slice(2, 3))} console=${lines.length}`);
  if (secs % 30 < 5) await shot(`boot-${secs}`);
  if (lines.some((l) => /loading screen: cleared/.test(l))) { console.log(`world ready after ${secs}s`); ready = true; break; }
  if (lines.some((l) => /panicked|RuntimeError|unreachable/.test(l))) { console.log('panic'); break; }
}
await sleep(3000); await shot('world-idle');
console.log('fps idle', await fps(5000));
await send('Input.dispatchMouseEvent', { type: 'mouseMoved', x: 400, y: 300 });
await key('rawKeyDown', 'w', 'KeyW', 87);
for (let i = 0; i < 4; i++) { await sleep(2500); await shot(`w-${i}`); }
console.log('fps moving', await fps(5000));
await key('keyUp', 'w', 'KeyW', 87);
await sleep(1000); await shot('after');
fs.writeFileSync(`${OUT}/console.log`, lines.join('\n'));
console.log(`console lines: ${lines.length} (${lines.filter((l) => /^\[(exception|log:error)/.test(l)).length} errors)`);
done(ready ? 0 : 3);
