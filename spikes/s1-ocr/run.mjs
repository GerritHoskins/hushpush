import { readFile, writeFile, mkdir, stat } from 'node:fs/promises';
import { spawn, spawnSync } from 'node:child_process';
import { homedir, cpus } from 'node:os';
import { join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { createHash } from 'node:crypto';
import { PNG } from 'pngjs';
import { root, out } from './paths.mjs';

const cargo = join(homedir(), '.cargo', 'bin', process.platform === 'win32' ? 'cargo.exe' : 'cargo');
const cli = join(homedir(), '.cargo', 'bin', process.platform === 'win32' ? 'ocrs.exe' : 'ocrs');
const cache = join(homedir(), '.cache', 'ocrs');
function command(exe, args) {
  return new Promise((resolve, reject) => {
    const start = performance.now();
    const child = spawn(exe, args, { cwd: root, stdio: ['ignore', 'pipe', 'pipe'] });
    // Do not emit child output: OCR failures might contain recognized content.
    child.stdout.resume(); child.stderr.resume();
    child.on('error', reject);
    child.on('close', code => code === 0 ? resolve((performance.now() - start) / 1000) : reject(new Error(`child exited ${code}`)));
  });
}

export function preprocess(source, dark) {
  const target = new PNG({ width: source.width * 2, height: source.height * 2 });
  // Bilinear interpolation on the existing screenshot, not a fresh DOM render.
  for (let y = 0; y < target.height; y++) for (let x = 0; x < target.width; x++) {
    const sx = Math.max(0, Math.min(source.width - 1, (x + 0.5) / 2 - 0.5));
    const sy = Math.max(0, Math.min(source.height - 1, (y + 0.5) / 2 - 0.5));
    const x0 = Math.floor(sx), y0 = Math.floor(sy), x1 = Math.min(x0 + 1, source.width - 1), y1 = Math.min(y0 + 1, source.height - 1);
    for (let c = 0; c < 3; c++) {
      const a = source.data[(y0 * source.width + x0) * 4 + c] * (1 - (sx - x0)) + source.data[(y0 * source.width + x1) * 4 + c] * (sx - x0);
      const b = source.data[(y1 * source.width + x0) * 4 + c] * (1 - (sx - x0)) + source.data[(y1 * source.width + x1) * 4 + c] * (sx - x0);
      const value = Math.round(a * (1 - (sy - y0)) + b * (sy - y0));
      target.data[(y * target.width + x) * 4 + c] = dark ? 255 - value : value;
    }
    target.data[(y * target.width + x) * 4 + 3] = 255;
  }
  return target;
}

async function main() {
  const manifest = JSON.parse(await readFile(`${out}/manifest.json`));
  await command(cargo, ['build', '--release', '--locked', '--manifest-path', 'bench/Cargo.toml']);
  // Separate acquisition from the cold-process measurement; cache downloads are not OCR time.
  await command(cli, [`${out}/${manifest.images[0].id}.png`, '--json', '-o', `${out}/probe.json`]);
  const models = [];
  for (const name of ['text-detection.onnx', 'text-recognition.onnx']) {
    const bytes = await readFile(join(cache, name));
    models.push({ name, bytes: (await stat(join(cache, name))).size, sha256: createHash('sha256').update(bytes).digest('hex') });
  }
  let physicalCores = null;
  if (process.platform === 'win32') {
    const probe = spawnSync('powershell.exe', ['-NoProfile', '-Command', '(Get-CimInstance Win32_Processor | Measure-Object -Property NumberOfCores -Sum).Sum'], { encoding: 'utf8' });
    if (probe.status === 0) physicalCores = Number(probe.stdout.trim());
  }
  await writeFile(`${out}/environment.json`, JSON.stringify({ cpu: cpus()[0].model.trim(), physicalCores, logicalCores: cpus().length, node: process.version, models, browser: manifest.browser, platform: process.platform, arch: process.arch }, null, 2));
  for (const variant of ['base', 'fallback']) {
    await mkdir(`${out}/${variant}`, { recursive: true });
    const timings = [], images = [];
    for (const [index, item] of manifest.images.entries()) {
      let path = `${out}/${item.id}.png`;
      if (variant === 'fallback') {
        path = `${out}/fallback/${item.id}.png`;
        const png = preprocess(PNG.sync.read(await readFile(`${out}/${item.id}.png`)), ['terminal-dark', 'devtools'].includes(item.template));
        await writeFile(path, PNG.sync.write(png));
      }
      const seconds = await command(cli, [path, '--json', '-o', `${out}/${variant}/${item.id}.ocr.json`]);
      timings.push({ id: item.id, seconds }); images.push(path);
      await writeFile(`${out}/${variant}/cold.json`, JSON.stringify(timings, null, 2));
      if ((index + 1) % 10 === 0) console.log(`${variant}: ${index + 1}/${manifest.images.length} CLI images`);
    }
    const bench = join(root, 'bench', 'target', 'release', process.platform === 'win32' ? 's1-ocr-bench.exe' : 's1-ocr-bench');
    await command(bench, [join(cache, models[0].name), join(cache, models[1].name), `${out}/${variant}/warm.json`, ...images]);
    console.log(`${variant}: warm benchmark complete`);
  }
}
if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  main().catch(() => { console.error('S1 run failed; child diagnostics suppressed to protect OCR text.'); process.exitCode = 1; });
}
