import { readFile, writeFile } from 'node:fs/promises';
import { out, root } from './paths.mjs';
import { Sensitive, findings, canonical, coverage } from './metrics.mjs';

const read = async path => {
  try { return JSON.parse(await readFile(path)); }
  catch { throw new Error('Required synthetic artifact missing or invalid; run npm run run to completion first.'); }
};
const manifest = await read(`${out}/manifest.json`);
const environment = await read(`${out}/environment.json`);
function diagnose(plant, ocr, factor) {
  const overlapping = ocr.paragraphs.flatMap(p => p.lines.flatMap(l => l.words)).filter(w => {
    const xs = w.vertices.map(p => p[0] / factor), ys = w.vertices.map(p => p[1] / factor);
    return coverage(plant.glyphBoxes, [{ x: Math.min(...xs), y: Math.min(...ys), width: Math.max(...xs) - Math.min(...xs), height: Math.max(...ys) - Math.min(...ys) }], 0) > 0;
  });
  if (!overlapping.length) return 'No OCR word overlaps planted glyph ranges';
  const local = new Sensitive(overlapping.map(w => w.text).join(' '));
  if (plant.category === 'email' && !local.expose().includes('@')) return 'Email @ absent from overlapping OCR words';
  if (plant.category === 'email' && !local.expose().includes('.')) return 'Email dot absent from overlapping OCR words';
  if (canonical(plant.category, local.expose()).includes(canonical(plant.category, plant.value))) return 'Characters survive normalization; separator or boundary mismatch';
  return 'Canonical characters altered in overlapping OCR words';
}
const all = [];
for (const variant of ['base', 'fallback']) {
  const cold = await read(`${out}/${variant}/cold.json`), warm = await read(`${out}/${variant}/warm.json`);
  if (cold.length !== manifest.images.length || warm.length !== manifest.images.length) throw new Error('Incomplete timing run');
  for (const image of manifest.images) {
    const truth = await read(`${out}/${image.id}.json`), ocr = await read(`${out}/${variant}/${image.id}.ocr.json`);
    for (const level of ['word', 'line']) {
      const detected = findings(ocr, variant === 'fallback' ? 2 : 1, level), used = new Set();
      const plants = truth.plants.filter(p => p.category !== 'benign').map(p => {
        const i = detected.findIndex((f, i) => !used.has(i) && f.category === p.category && canonical(p.category, f.value.expose()) === canonical(p.category, p.value));
        if (i >= 0) used.add(i);
        return { id: p.id, category: p.category, found: i >= 0, reason: i >= 0 ? null : diagnose(p, ocr, variant === 'fallback' ? 2 : 1), coverage: [0, 4, 8].map(padding => i < 0 ? 0 : coverage(p.glyphBoxes, detected[i].rects, padding)) };
      });
      const falseFindings = detected.filter((_, i) => !used.has(i)).map(f => {
        const onLookalike = truth.plants.filter(p => p.category === 'benign').some(p => coverage(p.glyphBoxes, f.rects, 0) > 0);
        return { category: f.category, onLookalike };
      });
      all.push({ ...image, variant, level, plants, falseFindings, cold: cold.find(t => t.id === image.id).seconds, warm: warm.find(t => t.id === image.id).seconds });
    }
  }
}
await writeFile(`${out}/scores.json`, JSON.stringify(all, null, 2));
const mean = a => a.reduce((s, n) => s + n, 0) / a.length;
const pct = n => `${(n * 100).toFixed(2)}%`;
const table = (head, rows) => `| ${head.join(' | ')} |\n| ${head.map(() => '---').join(' | ')} |\n${rows.map(r => `| ${r.join(' | ')} |`).join('\n')}\n`;
const selected = (variant, scale, level = 'word') => all.filter(r => r.variant === variant && r.scale === scale && r.level === level);
const rows = [];
for (const variant of ['base', 'fallback']) for (const scale of [1, 1.5, 2]) for (const category of ['email', 'phone', 'card']) {
  const images = selected(variant, scale), p = images.flatMap(r => r.plants).filter(p => p.category === category), found = p.filter(p => p.found).length;
  rows.push([variant, scale, category, p.length, found, pct(found / p.length), images.flatMap(r => r.falseFindings).filter(f => f.category === category && f.onLookalike).length, images.flatMap(r => r.falseFindings).filter(f => f.category === category).length]);
}
const localization = [];
for (const variant of ['base', 'fallback']) for (const scale of [1, 1.5, 2]) for (const level of ['word', 'line']) {
  const p = selected(variant, scale, level).flatMap(r => r.plants), found = p.filter(p => p.found);
  localization.push([variant, scale, level, found.length, ...[0, 1, 2].map(i => pct(mean(found.map(p => p.coverage[i])))), pct(Math.min(...found.map(p => p.coverage[2]))), `${found.filter(p => p.coverage[2] >= .98).length}/${found.length}`, pct(mean(p.map(p => p.coverage[2])))]);
}
const timings = [];
for (const variant of ['base', 'fallback']) for (const scale of [1, 1.5, 2]) {
  const images = selected(variant, scale);
  timings.push([variant, scale, images.length, ...['cold', 'warm'].flatMap(k => [mean(images.map(i => i[k])).toFixed(3), Math.max(...images.map(i => i[k])).toFixed(3)])]);
}
const breakdown = [];
for (const variant of ['base', 'fallback']) for (const template of [...new Set(manifest.images.map(i => i.template))]) for (const font of [11, 12, 14, 16]) {
  const p = all.filter(r => r.variant === variant && r.level === 'word' && r.template === template && r.font === font).flatMap(r => r.plants);
  breakdown.push([variant, template, font, p.length, p.filter(p => p.found).length, pct(p.filter(p => p.found).length / p.length)]);
}
const failures = all.filter(r => r.level === 'word' && (r.plants.some(p => !p.found || p.coverage[2] < .98) || r.falseFindings.length)).map(r => [r.variant, r.id, r.plants.filter(p => !p.found).map(p => `${p.id}(${p.category})`).join(', ') || 'none', r.plants.filter(p => p.found && p.coverage[2] < .98).map(p => `${p.id}: ${pct(p.coverage[2])}`).join(', ') || 'none', r.falseFindings.length]);
const gates = [];
for (const variant of ['base', 'fallback']) {
  for (const scale of [1, 2]) {
    const images = selected(variant, scale);
    const pass = ['email', 'phone', 'card'].every(category => { const p = images.flatMap(r => r.plants).filter(p => p.category === category); return p.filter(p => p.found).length / p.length >= (scale === 1 ? .85 : .95); });
    gates.push([variant, `recall at ${scale}x (each category)`, pass ? 'PASS' : 'FAIL']);
  }
  const p = all.filter(r => r.variant === variant && r.level === 'word').flatMap(r => r.plants).filter(p => p.found);
  gates.push([variant, 'every found value ≥98% glyph-range coverage, word boxes +8 px', p.every(p => p.coverage[2] >= .98) ? 'PASS' : 'FAIL']);
  gates.push([variant, 'every 1x image warm ≤1.5 s on measured desktop', selected(variant, 1).every(r => r.warm <= 1.5) ? 'PASS' : 'FAIL']);
  gates.push([variant, 'warm ≤1.5 s on a 4-core laptop', 'NOT RUN']);
}
const deltas = [];
for (const scale of [1, 1.5, 2]) for (const category of ['email', 'phone', 'card']) {
  const counts = ['base', 'fallback'].map(v => selected(v, scale).flatMap(r => r.plants).filter(p => p.category === category));
  const recalls = counts.map(p => p.filter(p => p.found).length / p.length);
  deltas.push([scale, category, ...recalls.map(pct), `${((recalls[1] - recalls[0]) * 100).toFixed(2)} pp`]);
}
const report = `# S1: ocrs screenshot PII experiment

Measured locally on ${new Date().toISOString().slice(0, 10)}. This is a bounded synthetic-corpus result, not a real-scene accuracy claim.

## Verdict

${table(['Variant', 'Criterion', 'Result'], gates)}
**S1 verdict: FAIL.** Baseline and first fallback miss recall and localization targets, so S1 does not clear the T4 gate. The four-core laptop performance criterion remains unverified.

## Method and corpus

${manifest.images.length} images: five templates × three device scales × four fonts, viewport 1920×1080 CSS pixels. Physical dimensions are 1920×1080, 2880×1620 and 3840×2160. Each image has three emails, two phones, three published test cards and five benign look-alikes. Reserved example-domain emails deliberately remain detector-positive for this spike, bypassing the product's reserved-domain suppression. Phone rules support the exact synthetic formats requested; they do not establish international phone coverage.

Seeded xorshift32; Edge ${manifest.browser}; Node ${environment.node}; pinned npm dependencies. Card fixtures are published by [Stripe](https://docs.stripe.com/testing#cards) and [Braintree](https://developer.paypal.com/braintree/docs/reference/general/testing/node/). Ground truth stores span ranges and individual non-whitespace character ranges from Range.getClientRects(), scaled by devicePixelRatio. “Glyph area” here means the union of these browser character rectangles, including font ascent/descent and advance width, not a bitmap ink mask. Fixtures are horizontally arranged, unwrapped, high contrast, with a modest text density; rotated text and realistic full-screen dense logs remain outside this result.

OCR text is reconstructed from word text with normalized whitespace and an offset-to-word map. Word-internal whitespace and word separators become spaces; line boundaries remain newlines to avoid merging unrelated numeric records. Email separators allow OCR whitespace; phones accept spaces around separators; cards allow spaces/hyphens and require Luhn, checking shorter legal endpoints if a following numeric token extends a candidate. Cards broken across OCR lines are a detector limitation. Matching canonicalizes whitespace for email and digits for phones/cards, with one-to-one occurrence assignment. No OCR spelling correction or ground-truth-guided text repair. False findings are unmatched detector outputs; the look-alike subset intersects a benign ground-truth range. Outputs contain category/plant IDs and metrics, never recognized values; only the explicitly requested synthetic truth and raw CLI JSON retain synthetic strings under ignored out/.

## Recall and false findings

${table(['Variant', 'Scale', 'Category', 'Planted', 'Found', 'Recall', 'False on look-alikes', 'All false findings'], rows)}
“Precision on look-alikes” is reported as the requested false-finding count, not a precision ratio.

## Localization

OCR quadrilaterals become axis-aligned redaction rectangles. Coverage is exact rectangle-union intersection area divided by union ground-truth character-range area; overlapping OCR boxes are counted once. Fallback coordinates map back to original device pixels before padding. Padding is in original device pixels. Both word and line boxes are evaluated; only words producing the detector match contribute in word mode, and their parent lines in line mode. Means are per-value means.

${table(['Variant', 'Scale', 'Boxes', 'Found', 'Mean +0 px', 'Mean +4 px', 'Mean +8 px', 'Worst +8 px', '≥98% at +8', 'End-to-end mean +8 (misses=0)'], localization)}
The strict localization verdict requires every found value to reach 98%; detection misses are separately reflected in recall and end-to-end coverage. Line boxes are a separate S1 fallback observation, not evidence that word localization passed.

## Timing and hardware

CPU: ${environment.cpu}; ${environment.physicalCores ?? "unknown"} physical cores / ${environment.logicalCores} logical processors (Win32_Processor). Windows ${environment.arch}. Default runtime threading, no CPU affinity limit. Release build; ocrs-cli/ocrs 0.13.1 and rten 0.26.0. One sequential measurement per image per mode, no concurrent OCR processes.

${table(['Variant', 'Original scale', 'Images', 'Cold mean s', 'Cold max s', 'Warm mean s', 'Warm max s'], timings)}
Cold means a fresh CLI process with cached models: process startup, model load, image decode, OCR and JSON write; it does not mean an evicted filesystem cache and excludes initial model downloads. Warm uses the Rust API, loads both models once from bytes, decodes each PNG outside timing, warms up once on the first image, then times prepare_input → detect_words → find_text_lines → recognize_text. No JSON/text serialization in warm timing. Fallback preprocessing and PNG decode are excluded from warm timing; fallback OCR inputs have twice the original width and height, reaching 7680×4320 at original 2x. The final timing refresh ran without this spike's builds or screenshot generation in parallel; other host background activity was not controlled. These samples are descriptive, not benchmark confidence intervals. The verdict uses the maximum observed sample, so a fast mean cannot hide a slow image.

## First fallback and delta

All original screenshots were bilinearly upscaled 2×; terminal-dark and devtools additionally had RGB inverted. This combined first fallback preserves font rasterization artifacts from the original screenshots. It does not isolate inversion from upscaling.

${table(['Original scale', 'Category', 'Baseline recall', 'Fallback recall', 'Delta'], deltas)}
## Failure breakdown

${table(['Variant', 'Template', 'CSS font px', 'Planted (all scales)', 'Found', 'Recall'], breakdown)}
Per-image exceptions below list missed plant IDs and poorly covered matched values. Use ignored out/<ID>.json to locate a plant without exposing its value in this report. No exceptions means no observed miss, under-coverage or false finding in that image.

${table(['Variant', 'Image ID', 'Missed IDs (category)', 'Found but <98% coverage at +8', 'False findings'], failures)}
## Observed CLI JSON contract

Actual keys: image_height, image_width, paragraphs, url. paragraphs[].lines[] has text, vertices, words; words[] has text and vertices. Both word and line polygons contain four [x,y] integer vertices. Coordinates align with original raster/device pixels (verified across scales against the planted ranges); they are not CSS coordinates. No confidence scores observed. The CLI groups all lines into one paragraph.

**Metadata bug in installed 0.13.1:** a 1920×1080 PNG reports image_width=3, image_height=1920. The scorer ignores those fields and uses PNG/ground-truth geometry. Word coordinates themselves remain usable; consumers must validate dimensions against their decoder. Raw CLI JSON is in out/{base,fallback}/<ID>.ocr.json.

## S3 notes

${table(['Cached model filename', 'Measured bytes', 'SHA-256'], environment.models.map(m => [m.name, m.bytes, m.sha256]))}
The CLI fetched these ONNX files from https://ocrs-models.s3-accelerate.amazonaws.com/ into ~/.cache/ocrs on first invocation. Model acquisition was completed before timed runs. No model files are committed here. Combined uncompressed size: ${environment.models.reduce((s, m) => s + m.bytes, 0)} bytes. Compressed release archive size and network-disabled execution: **not measured**.

The bench actually loads both files via Model::load(Vec<u8>) and passes them into OcrEngineParams, proving byte-loading on these exact versions. [RTen Model API](https://docs.rs/rten/0.26.0/rten/struct.Model.html#method.load) documents byte loading and load_static_slice; [ocrs OcrEngineParams](https://docs.rs/ocrs/0.13.1/ocrs/struct.OcrEngineParams.html) accepts optional rten::Model instances for detection and recognition. Thus embedded bytes are feasible; a shipping binary still needs independent packaging and offline validation.

The [upstream model card](https://huggingface.co/robertknight/ocrs) explicitly labels pretrained weights **CC-BY-SA-4.0**, and says training uses HierText and synthetic data. The [training repository](https://github.com/robertknight/ocrs-models#datasets) identifies HierText as CC-BY-SA-4.0. These terms differ from the library's MIT/Apache-2.0 source licenses. The S3-download endpoint itself provided no accompanying model license during this run; equivalence of the exact S3 weights to the model-card files was not established. Redistribution under the repository's permissive-only policy is therefore **unresolved**, not approved by this spike.

## Evidence and reproducibility

See README.md for the three-command run. out/manifest.json holds PNG and truth hashes; environment.json records CPU/models/browser; {base,fallback}/{cold,warm}.json hold per-image timings; scores.json holds masked scoring evidence. The generator reproducibility check, tests, and Rust quality gates are recorded in VALIDATION.md.
`;
const diagnoses = [];
for (const variant of ['base', 'fallback']) {
  const images = all.filter(r => r.variant === variant && r.level === 'word');
  const reasons = [...new Set(images.flatMap(r => r.plants.filter(p => !p.found).map(p => p.reason)))];
  for (const reason of reasons) {
    const examples = images.filter(r => r.plants.some(p => p.reason === reason));
    diagnoses.push([variant, reason, images.flatMap(r => r.plants).filter(p => p.reason === reason).length, examples.slice(0, 3).map(r => r.id).join(', ')]);
  }
}
const annotatedReport = report.replace('## Failure breakdown', '## Observed failure modes\n\n' + table(['Variant', 'Observed text/geometry symptom', 'Missed values', 'Example image IDs'], diagnoses) + '\nSymptoms come from words geometrically overlapping each planted glyph range. They distinguish absent email punctuation and altered canonical characters from detector separator/boundary failures; they do not infer an OCR model root cause. The font/template table below shows where these occur. Scaling does not monotonically improve this corpus; the combined fallback improves 1x phone/card recall but does not reach the full gate.\n\n## Failure breakdown');
const categoryLocalization = [];
for (const variant of ['base', 'fallback']) for (const scale of [1, 1.5, 2]) for (const category of ['email', 'phone', 'card']) {
  const found = selected(variant, scale).flatMap(r => r.plants).filter(p => p.category === category && p.found);
  categoryLocalization.push([variant, scale, category, found.length, ...[0, 1, 2].map(i => pct(mean(found.map(p => p.coverage[i])))), pct(Math.min(...found.map(p => p.coverage[2])))]);
}
const finalReport = annotatedReport.replace('The strict localization verdict', 'Word-box localization by category:\n\n' + table(['Variant', 'Scale', 'Category', 'Found', 'Mean +0 px', 'Mean +4 px', 'Mean +8 px', 'Worst +8 px'], categoryLocalization) + '\nThe strict localization verdict');
await writeFile(`${root}/RESULTS.md`, finalReport);
console.log('Scored both variants; wrote masked RESULTS.md and out/scores.json');
