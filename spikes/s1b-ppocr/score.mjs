import { readFile, writeFile, mkdir } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { resolve } from 'node:path';
import { Sensitive, canonical, coverage, findings, rect } from './metrics.mjs';

const root = fileURLToPath(new URL('.', import.meta.url));
const corpus = resolve(root, '../s1-ocr/out');
const read = async path => {
  try { return JSON.parse(await readFile(path)); }
  catch { throw new Error(`Missing or invalid required artifact: ${path}`); }
};
const args = process.argv.slice(2);
if (!args.length || args.some(v => !/^[a-z0-9-]+$/.test(v))) throw new Error('Usage: node score.mjs mobile [server]; reads only local synthetic corpus and out/<variant>');
const manifest = await read(`${corpus}/manifest.json`);
const baseline = await read(`${corpus}/scores.json`);
const all = [];
for (const variant of args) {
  const directory = `${root}/out/${variant}`;
  const timings = await read(`${directory}/warm.json`);
  if (timings.length !== manifest.images.length || new Set(timings.map(t => t.id)).size !== manifest.images.length) throw new Error('Incomplete or duplicate timing run');
  for (const image of manifest.images) {
    const truth = await read(`${corpus}/${image.id}.json`), ocr = await read(`${directory}/${image.id}.ocr.json`);
    const timing = timings.find(t => t.id === image.id);
    if (!timing || !Number.isFinite(timing.seconds) || timing.seconds < 0) throw new Error('Missing or invalid warm timing');
    if (ocr.image_width !== truth.width || ocr.image_height !== truth.height) throw new Error('OCR dimensions disagree with original device dimensions');
    for (const level of ['char', 'line']) {
      const detected = findings(ocr, level), used = new Set();
      const plants = truth.plants.filter(p => p.category !== 'benign').map(p => {
        const value = new Sensitive(p.value);
        const index = detected.findIndex((f, i) => !used.has(i) && f.category === p.category && canonical(p.category, f.value.expose()) === canonical(p.category, value.expose()));
        if (index >= 0) used.add(index);
        let reason = null;
        if (index < 0) {
          const lines = ocr.paragraphs.flatMap(p => p.lines).filter(l => coverage(p.glyphBoxes, [rect(l.vertices)], 0) > 0);
          const local = new Sensitive(lines.map(l => l.text).join(' '));
          reason = !lines.length ? 'No OCR line overlaps planted glyph ranges' : p.category === 'email' && !local.expose().includes('@') ? 'Email @ absent from overlapping OCR lines' : p.category === 'email' && !local.expose().includes('.') ? 'Email dot absent from overlapping OCR lines' : canonical(p.category, local.expose()).includes(canonical(p.category, value.expose())) ? 'Canonical value survives; detector separator or boundary mismatch' : 'Canonical characters altered in overlapping OCR lines';
        }
        return { id: p.id, category: p.category, found: index >= 0, reason, coverage: [0, 4, 8].map(padding => index < 0 ? 0 : coverage(p.glyphBoxes, detected[index].rects, padding)) };
      });
      const falseFindings = detected.filter((_, i) => !used.has(i)).map(f => ({ category: f.category, onLookalike: truth.plants.filter(p => p.category === 'benign').some(p => coverage(p.glyphBoxes, f.rects, 0) > 0) }));
      all.push({ id: image.id, scale: image.scale, template: image.template, font: image.font, variant, level, plants, falseFindings, warm: timing.seconds });
    }
  }
}
await mkdir(`${root}/out`, { recursive: true });
await writeFile(`${root}/out/scores.json`, JSON.stringify(all, null, 2));
const mean = a => a.length ? a.reduce((s, n) => s + n, 0) / a.length : null;
const pct = n => n === null || !Number.isFinite(n) ? 'N/A' : `${(n * 100).toFixed(2)}%`;
const number = n => n === null ? 'N/A' : n.toFixed(3);
const table = (headers, rows) => `| ${headers.join(' | ')} |\n| ${headers.map(() => '---').join(' | ')} |\n${rows.map(r => `| ${r.join(' | ')} |`).join('\n')}\n`;
const selected = (variant, scale, level = 'char') => all.filter(r => r.variant === variant && r.scale === scale && r.level === level);
const baseSelected = (scale, level = 'char') => baseline.filter(r => r.variant === 'base' && r.scale === scale && r.level === (level === 'char' ? 'word' : 'line'));
const coverageCells = p => {
  const found = p.filter(p => p.found);
  return [...[0, 1, 2].map(i => pct(mean(found.map(p => p.coverage[i])))), pct(found.length ? Math.min(...found.map(p => p.coverage[2])) : null), `${found.filter(p => p.coverage[2] >= .98).length}/${found.length}`, pct(mean(p.map(p => p.coverage[2])))];
};
const gates = [], recalls = [], localization = [], categories = [], timingRows = [], breakdown = [], failures = [], diagnoses = [];
for (const variant of args) {
  for (const scale of [1, 1.5, 2]) {
    const images = selected(variant, scale);
    for (const category of ['email', 'phone', 'card']) {
      const p = images.flatMap(r => r.plants).filter(p => p.category === category), found = p.filter(p => p.found);
      const base = baseline.filter(r => r.variant === 'base' && r.level === 'word' && r.scale === scale).flatMap(r => r.plants).filter(p => p.category === category);
      const baseRecall = base.filter(p => p.found).length / base.length;
      recalls.push([variant, scale, category, p.length, found.length, pct(found.length / p.length), pct(baseRecall), `${((found.length / p.length - baseRecall) * 100).toFixed(2)} pp`, images.flatMap(r => r.falseFindings).filter(f => f.category === category && f.onLookalike).length, images.flatMap(r => r.falseFindings).filter(f => f.category === category).length]);
    }
    for (const level of ['char', 'line']) {
      const p = selected(variant, scale, level).flatMap(r => r.plants), found = p.filter(p => p.found);
      localization.push([variant, scale, level, found.length, ...coverageCells(p), ...coverageCells(baseSelected(scale, level).flatMap(r => r.plants))]);
      for (const category of ['email', 'phone', 'card']) {
        const subset = found.filter(p => p.category === category);
        const baseSubset = baseSelected(scale, level).flatMap(r => r.plants).filter(p => p.category === category && p.found);
        categories.push([variant, scale, level, category, subset.length, ...coverageCells(subset).slice(0, 4), ...coverageCells(baseSubset).slice(0, 4)]);
      }
    }
    const baseTimes = baseSelected(scale).map(i => i.warm);
    timingRows.push([variant, scale, images.length, number(mean(images.map(i => i.warm))), number(Math.max(...images.map(i => i.warm))), number(mean(baseTimes)), number(Math.max(...baseTimes))]);
  }
  for (const scale of [1, 2]) {
    const pass = ['email', 'phone', 'card'].every(category => { const p = selected(variant, scale).flatMap(r => r.plants).filter(p => p.category === category); return p.length && p.filter(p => p.found).length / p.length >= (scale === 1 ? .85 : .95); });
    const basePass = ['email', 'phone', 'card'].every(category => { const p = baseSelected(scale).flatMap(r => r.plants).filter(p => p.category === category); return p.length && p.filter(p => p.found).length / p.length >= (scale === 1 ? .85 : .95); });
    gates.push([variant, `recall at ${scale}x (each category)`, pass ? 'PASS' : 'FAIL', basePass ? 'PASS' : 'FAIL']);
  }
  for (const level of ['char', 'line']) {
    const p = all.filter(r => r.variant === variant && r.level === level).flatMap(r => r.plants).filter(p => p.found);
    const basePlants = baseline.filter(r => r.variant === 'base' && r.level === (level === 'char' ? 'word' : 'line')).flatMap(r => r.plants).filter(p => p.found);
    gates.push([variant, `every found value ≥98% glyph-range coverage, ${level} boxes +8 px`, p.length && p.every(p => p.coverage[2] >= .98) ? 'PASS' : 'FAIL', basePlants.length && basePlants.every(p => p.coverage[2] >= .98) ? 'PASS' : 'FAIL']);
  }
  gates.push([variant, 'every 1x image warm ≤1.5 s on measured desktop', selected(variant, 1).every(r => r.warm <= 1.5) ? 'PASS' : 'FAIL', baseSelected(1).every(r => r.warm <= 1.5) ? 'PASS' : 'FAIL']);
  gates.push([variant, 'warm ≤1.5 s on a 4-core laptop', 'NOT RUN', 'NOT RUN']);
  const images = all.filter(r => r.variant === variant && r.level === 'char');
  for (const reason of new Set(images.flatMap(r => r.plants.filter(p => !p.found).map(p => p.reason)))) {
    const examples = images.filter(r => r.plants.some(p => p.reason === reason));
    diagnoses.push([variant, reason, images.flatMap(r => r.plants).filter(p => p.reason === reason).length, examples.slice(0, 4).map(r => r.id).join(', ')]);
  }
  for (const template of new Set(images.map(i => i.template))) for (const font of [11, 12, 14, 16]) {
    const p = images.filter(r => r.template === template && r.font === font).flatMap(r => r.plants);
    breakdown.push([variant, template, font, p.length, p.filter(p => p.found).length, pct(p.filter(p => p.found).length / p.length)]);
  }
  for (const image of images) {
    const line = selected(variant, image.scale, 'line').find(r => r.id === image.id);
    if (image.plants.some(p => !p.found || p.coverage[2] < .98) || image.falseFindings.length || line.plants.some(p => p.found && p.coverage[2] < .98)) failures.push([variant, image.id, image.plants.filter(p => !p.found).map(p => `${p.id}(${p.category})`).join(', ') || 'none', ...[image, line].map(r => r.plants.filter(p => p.found && p.coverage[2] < .98).map(p => `${p.id}: ${pct(p.coverage[2])}`).join(', ') || 'none'), image.falseFindings.length]);
  }
}
const report = `## Measured gates\n\n${table(['Variant', 'Criterion', 'S1b', 'S1 base'], gates)}\nS1 base char comparisons use its word boxes; line comparisons use its line boxes. Four-core laptop remains unmeasured.\n\n## Recall and false findings\n\n${table(['Variant', 'Scale', 'Category', 'Planted', 'Found', 'Recall', 'S1 base', 'Delta', 'False on look-alikes', 'All false findings'], recalls)}\n## Localization\n\n${table(['Variant', 'Scale', 'Boxes', 'Found', 'Mean +0 px', 'Mean +4 px', 'Mean +8 px', 'Worst +8 px', '≥98% at +8', 'End-to-end mean +8 (misses=0)', 'S1 mean +0', 'S1 mean +4', 'S1 mean +8', 'S1 worst +8', 'S1 ≥98% at +8', 'S1 end-to-end +8'], localization)}\n${table(['Variant', 'Scale', 'Boxes', 'Category', 'Found', 'Mean +0 px', 'Mean +4 px', 'Mean +8 px', 'Worst +8 px', 'S1 mean +0', 'S1 mean +4', 'S1 mean +8', 'S1 worst +8'], categories)}\n## Warm timing\n\n${table(['Variant', 'Scale', 'Images', 'Warm mean s', 'Warm max s', 'S1 mean s', 'S1 max s'], timingRows)}\n## Observed failure modes\n\n${table(['Variant', 'Observed symptom', 'Missed values', 'Example image IDs'], diagnoses)}\nSymptoms examine overlapping full lines and can include neighboring tokens; they do not establish a model root cause.\n\n## Failure breakdown\n\n${table(['Variant', 'Template', 'CSS font px', 'Planted', 'Found', 'Recall'], breakdown)}\n## Per-image exceptions\n\n${table(['Variant', 'Image ID', 'Missed IDs (category)', 'Char <98% at +8', 'Line <98% at +8', 'False findings'], failures)}`;
await writeFile(`${root}/out/metrics-report.md`, report);
console.log(`Scored ${args.length} variant(s) on ${manifest.images.length} synthetic images; masked scores and metrics report written under out/.`);
