import { readFile, writeFile } from 'node:fs/promises';
import { spawnSync } from 'node:child_process';
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { root, out } from './paths.mjs';
const before = JSON.parse(await readFile(`${out}/manifest.json`));
const truthBytesBefore = new Map();
for (const item of before.images) truthBytesBefore.set(item.id, createHash('sha256').update(await readFile(`${out}/${item.id}.json`)).digest('hex'));
const result = spawnSync(process.execPath, ['gen/generate.mjs'], { cwd: root, stdio: 'inherit' });
assert.equal(result.status, 0);
const after = JSON.parse(await readFile(`${out}/manifest.json`));
assert.deepEqual(after, before);
// Validate both artifacts, not just the hashes claimed in the manifest.
for (const item of after.images) {
  assert.equal(createHash('sha256').update(await readFile(`${out}/${item.id}.png`)).digest('hex'), item.sha256);
  assert.equal(createHash('sha256').update(JSON.stringify(JSON.parse(await readFile(`${out}/${item.id}.json`)))).digest('hex'), item.truthSha256);
  assert.equal(createHash('sha256').update(await readFile(`${out}/${item.id}.json`)).digest('hex'), truthBytesBefore.get(item.id));
}
await writeFile(`${out}/reproducibility.json`, JSON.stringify({ imagesCompared: after.images.length, imageHashesEqual: true, truthHashesEqual: true, rawTruthBytesEqual: true, browser: after.browser }, null, 2));
console.log(`PASS: ${after.images.length} PNG hashes and truth hashes identical across two generations`);
