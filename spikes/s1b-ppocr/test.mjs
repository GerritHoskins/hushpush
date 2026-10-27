import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { createHash } from 'node:crypto';
import { findings, coverage, Sensitive } from './metrics.mjs';
import { findings as s1Findings } from '../s1-ocr/metrics.mjs';

const corpus = new URL('../s1-ocr/out/', import.meta.url);
const manifest = JSON.parse(await readFile(new URL('manifest.json', corpus)));
const truth = JSON.parse(await readFile(new URL(`${manifest.images[0].id}.json`, corpus)));
const polygon = (x, width = 10) => [[x, 0], [x + width, 0], [x + width, 20], [x, 20]];
function ocrFor(text) {
  return { paragraphs: [{ lines: [{ text, vertices: polygon(0, text.length * 10), chars: Array.from(text, (text, i) => ({ text, vertices: polygon(i * 10) })), words: [{ text, vertices: polygon(0, text.length * 10) }] }] }] };
}

test('CTC characters preserve S1 detection and keep char boxes tighter than lines', () => {
  for (const plant of truth.plants.filter(p => p.category !== 'benign')) {
    const text = new Sensitive(`prefix ${plant.value} suffix`), ocr = ocrFor(text.expose());
    const chars = findings(ocr), lines = findings(ocr, 'line'), original = s1Findings(ocr);
    assert.equal(chars.length, original.length);
    assert.equal(chars.length, lines.length);
    assert.ok(chars.length > 0);
    assert.equal(chars[0].category, original[0].category);
    assert.equal(chars[0].value.expose() === original[0].value.expose(), true);
    assert.ok(chars[0].rects.every(r => r.width === 10));
    assert.equal(lines[0].rects.length, 1);
    assert.ok(lines[0].rects[0].width > 10);
  }
});

test('Missing or mismatched CTC geometry fails rather than silently using whole lines', () => {
  const ocr = ocrFor(truth.plants[0].value);
  ocr.paragraphs[0].lines[0].chars.pop();
  assert.throws(() => findings(ocr), /Missing character geometry/);
  ocr.paragraphs[0].lines[0].chars[0].start = 1;
  assert.throws(() => findings(ocr), /offsets disagree/);
});

test('Whitespace normalization preserves exact matched substring geometry', () => {
  const value = new Sensitive(truth.plants[0].value), text = new Sensitive(`prefix\t  ${value.expose()}   suffix`);
  const ocr = ocrFor(text.expose());
  const detected = findings(ocr), line = findings(ocr, 'line');
  assert.equal(detected.length, 1);
  assert.equal(detected.length, line.length);
  assert.equal(detected[0].rects.length, value.expose().length);
  assert.equal(Math.min(...detected[0].rects.map(r => r.x)), 90);
  assert.equal(Math.max(...detected[0].rects.map(r => r.x + r.width)), (9 + value.expose().length) * 10);
  assert.equal(detected[0].value.expose() === line[0].value.expose(), true);
});

test('UTF-16 offsets support non-BMP dictionary entries before detector tokens', () => {
  const value = new Sensitive(truth.plants[0].value), text = new Sensitive(`😀 ${value.expose()}`);
  const ocr = ocrFor(text.expose());
  let offset = 0;
  for (const char of ocr.paragraphs[0].lines[0].chars) {
    char.start = offset; offset += char.text.length; char.end = offset;
  }
  assert.equal(findings(ocr).length, 1);
});

test('Coverage union counts overlap once and padding is in device pixels', () => {
  const glyphs = [{ x: 0, y: 0, width: 20, height: 20 }];
  const rects = [{ x: 4, y: 4, width: 12, height: 12 }];
  assert.equal(coverage(glyphs, [...rects, ...rects], 0), .36);
  assert.equal(coverage(glyphs, rects, 4), 1);
});

test('Sensitive values serialize and inspect masked', () => {
  const value = new Sensitive(truth.plants[0].value);
  assert.equal(String(value), '[redacted]');
  assert.equal(JSON.stringify(value), '"[redacted]"');
});

test('Existing S1 corpus PNG and compact truth hashes agree with manifest, read only', async () => {
  for (const image of manifest.images) {
    const png = await readFile(new URL(`${image.id}.png`, corpus));
    const truth = JSON.parse(await readFile(new URL(`${image.id}.json`, corpus)));
    assert.equal(createHash('sha256').update(png).digest('hex'), image.sha256);
    assert.equal(createHash('sha256').update(JSON.stringify(truth)).digest('hex'), image.truthSha256);
  }
});
