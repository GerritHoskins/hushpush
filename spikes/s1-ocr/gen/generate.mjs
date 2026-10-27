import { chromium } from 'playwright-core';
import { mkdir, writeFile, readFile } from 'node:fs/promises';
import { createHash } from 'node:crypto';
import { root, out } from '../paths.mjs';

let state = 0x51c0ffee;
const random = () => { state ^= state << 13; state ^= state >>> 17; state ^= state << 5; return (state >>> 0) / 2 ** 32; };
const templates = ['terminal-dark', 'terminal-light', 'chat', 'devtools', 'log-viewer'];
await mkdir(out, { recursive: true });
const browser = await chromium.launch({ channel: 'msedge', headless: true });
const manifest = [];
try {
  for (const template of templates) for (const scale of [1, 1.5, 2]) for (const font of [11, 12, 14, 16]) {
    const id = `${template}-s${scale}-f${font}`;
    const context = await browser.newContext({ viewport: { width: 1920, height: 1080 }, deviceScaleFactor: scale, locale: 'en-US', timezoneId: 'UTC', colorScheme: 'light' });
    const page = await context.newPage();
    const n = Math.floor(random() * 9000) + 1000;
    const plants = [
      ['email', `dev${n}@example.com`], ['email', `qa${n}@example.org`], ['email', `ops${n}@example.net`],
      ['phone', `+1 555-01${n % 100 < 10 ? '0' : ''}${n % 100}-${n}`], ['phone', `(555) 01${n % 10}-${n}`],
      ['card', '4242 4242 4242 4242'], ['card', '4111-1111-1111-1111'], ['card', '5555 5555 5555 4444'],
      ['benign', 'user@localhost'], ['benign', '1.24.16'], ['benign', '2026-01-02T12:34:56Z'], ['benign', 'deadbeef01234567'],
      ['benign', '4242 4242 4242 4241'],
    ].map(([category, value], i) => ({ id: `p${i}`, category, value }));
    const dark = template === 'terminal-dark' || template === 'devtools';
    const header = { 'terminal-dark': 'Terminal — synthetic build output', 'terminal-light': 'Terminal — synthetic build output', chat: 'Team Chat / #synthetic-support', devtools: 'DevTools / Console / synthetic.local', 'log-viewer': 'Log Viewer / Synthetic events / INFO' }[template];
    const labels = ['account email', 'review email', 'service email', 'contact phone', 'callback phone', 'test payment', 'test payment', 'test payment', 'code const login =', 'version', 'timestamp', 'trace id', 'invalid checksum'];
    const rows = plants.map((p, i) => `<div class="row"><b>${template === 'chat' ? 'Demo Bot' : '12:34:56 INFO'}</b> ${labels[i]}: <span data-id="${p.id}">${p.value}</span></div>`).join('');
    await page.setContent(`<!doctype html><html><head><style>*{box-sizing:border-box}html,body{margin:0;width:1920px;height:1080px;background:${dark ? '#181b20' : '#f6f7f9'};color:${dark ? '#e4e8ed' : '#202630'};font:${font}px ${template === 'chat' ? 'Arial' : 'Consolas'},monospace}header{height:64px;padding:20px;background:${dark ? '#292e36' : '#e0e4ea'};border-bottom:1px solid #7e8792}main{margin:28px 40px}.row{padding:9px 12px;line-height:1.5;white-space:pre-wrap;${template === 'chat' ? 'margin:8px 0;background:#fff;border-radius:8px;width:1100px;' : ''}${template === 'log-viewer' ? 'border-bottom:1px solid #d6dbe2;' : ''}}b{font-weight:normal;color:${dark ? '#9eb5cb' : '#50677e'};margin-right:18px}</style></head><body><header>${header}</header><main>${rows}<div class="row">Synthetic fixture only. No production data.</div></main></body></html>`);
    await page.evaluate(() => document.fonts.ready);
    const boxes = await page.evaluate(() => [...document.querySelectorAll('[data-id]')].map(span => {
      const node = span.firstChild;
      const rect = r => ({ x: r.x * devicePixelRatio, y: r.y * devicePixelRatio, width: r.width * devicePixelRatio, height: r.height * devicePixelRatio });
      const range = document.createRange(); range.selectNodeContents(span);
      const glyphBoxes = [];
      for (let i = 0; i < node.length; i++) { if (/\s/.test(node.textContent[i])) continue; range.setStart(node, i); range.setEnd(node, i + 1); glyphBoxes.push(...[...range.getClientRects()].map(rect)); }
      range.selectNodeContents(span);
      return { id: span.dataset.id, boxes: [...range.getClientRects()].map(rect), glyphBoxes };
    }));
    await page.screenshot({ path: `${out}/${id}.png`, animations: 'disabled' });
    const truth = { id, template, scale, font, width: 1920 * scale, height: 1080 * scale, plants: plants.map((p, i) => ({ ...p, ...boxes[i] })) };
    await writeFile(`${out}/${id}.json`, JSON.stringify(truth, null, 2) + '\n');
    manifest.push({ id, template, scale, font, sha256: createHash('sha256').update(await readFile(`${out}/${id}.png`)).digest('hex'), truthSha256: createHash('sha256').update(JSON.stringify(truth)).digest('hex') });
    await context.close();
  }
  await writeFile(`${out}/manifest.json`, JSON.stringify({ seed: '0x51c0ffee', browser: browser.version(), images: manifest }, null, 2) + '\n');
  console.log(`Generated ${manifest.length} synthetic images in ${root}/out`);
} finally { await browser.close(); }
