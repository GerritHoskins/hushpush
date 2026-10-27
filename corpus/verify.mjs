import fs from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { createHash } from 'node:crypto';
import assert from 'node:assert/strict';

const split = process.argv.includes('--split') ? process.argv[process.argv.indexOf('--split') + 1] : 'dev';
const root = path.join(path.dirname(fileURLToPath(import.meta.url)), ...(split === 'test' ? ['test'] : []));
const manifest = JSON.parse(await fs.readFile(path.join(root, 'labels.json')));
assert.equal(manifest.seed, split === 'test' ? 20261028 : 20261027);
assert.equal(manifest.version, 2);
const counts = new Map();
const contexts = new Map();
let images = 0;
let clipboard = 0;
let multiline = 0;
let physicalMultiline = 0;
const matrix = new Set();
for (const file of manifest.files) {
  const location = path.join(root, file.path);
  assert.ok(location.startsWith(root + path.sep));
  if (file.symlink_target) {
    assert.equal(await fs.readlink(location), file.symlink_target);
    continue;
  }
  const bytes = await fs.readFile(location);
  assert.equal(createHash('sha256').update(bytes).digest('hex'), file.sha256, `hash: ${file.path}`);
  if (file.kind === 'text') {
    contexts.set(file.context, (contexts.get(file.context) || 0) + 1);
    assert.ok(file.negative_lines >= file.positive_lines, `negative support: ${file.path}`);
    if (file.path.endsWith('.json')) JSON.parse(bytes.toString());
    if (file.path.endsWith('.jsonl')) for (const line of bytes.toString().trim().split('\n')) JSON.parse(line);
  }
  if (file.kind === 'image') {
    images++;
    matrix.add(`${file.template}/${file.scale}`);
    assert.ok(file.font_px >= 11 && file.font_px <= 16);
    assert.equal(bytes.readUInt32BE(16), 960 * file.scale);
    assert.equal(bytes.readUInt32BE(20), 540 * file.scale);
  }
  for (const label of file.labels) {
    if (file.kind === 'text') {
      counts.set(label.category, (counts.get(label.category) || 0) + 1);
      assert.ok(label.core.start >= 0 && label.core.end > label.core.start && label.core.end <= bytes.length, 'span bounds');
      assert.ok(label.extent.start <= label.core.start && label.extent.end >= label.core.end, 'core inside extent');
      // UTF-8 byte boundaries must survive Windows checkout unchanged.
      assert.ok(!bytes.subarray(label.core.start, label.core.end).toString().includes('\ufffd'), 'UTF-8 boundaries');
      if (label.category === 'clipboard_record') { clipboard++; if (label.multiline) multiline++; if (bytes.subarray(label.core.start, label.core.end).includes(10)) physicalMultiline++; }
    } else assert.ok(label.core.boxes.length > 0);
  }
}
assert.equal(contexts.size, 8);
for (const count of contexts.values()) assert.ok(count >= 8);
for (const category of ['email', 'payment_card', 'phone', 'home_path', 'private_key', 'clipboard_record', 'user_pattern']) assert.ok(counts.get(category) >= 40, category);
assert.ok(images >= 48);
assert.equal(matrix.size, 16);
assert.ok(multiline / clipboard >= 0.4);
assert.ok(physicalMultiline / clipboard >= 0.4);
const phones = manifest.files.flatMap(f => f.labels).filter(l => l.country);
for (const country of ['DE', 'GB', 'FR', 'NL', 'ES', 'IT', 'JP', 'AU', 'US']) assert.ok(phones.some(p => p.country === country), country);
const listed = new Set(manifest.files.map(f => f.path));
for (const directory of ['text', 'images', 'edge']) {
  for (const entry of await fs.readdir(path.join(root, directory), { recursive: true, withFileTypes: true })) {
    if (entry.isDirectory()) continue;
    const actual = path.relative(root, path.join(entry.parentPath, entry.name)).replaceAll('\\', '/');
    assert.ok(listed.has(actual), `unlisted fixture: ${actual}`);
  }
}
const added = manifest.files.flatMap(f => f.labels).filter(l => l.format === 'e164-national-format');
assert.ok(added.length >= 40);
assert.equal(new Set(added.map(l => l.country)).size, 8);
for (const group of ['phone/national-no-country-code', 'clipboard/undocumented-key']) assert.ok(manifest.files.some(f => f.labels.some(l => l.off_spec === group)));

for (const key of ['clipboard', 'pasteboard', 'clip_text', 'primaryClip', 'copiedText']) assert.ok(manifest.files.some(f => f.labels.some(l => l.key === key)));

console.log(`PASS: ${manifest.files.length} fixture hashes/spans, ${images} images, 8 text contexts, 7 positive categories, ${physicalMultiline}/${clipboard} physically multiline clipboard records (${multiline} before JSON escaping).`);
console.log(JSON.stringify(Object.fromEntries(counts)));

