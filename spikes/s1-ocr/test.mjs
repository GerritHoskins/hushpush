import test from 'node:test';
import assert from 'node:assert/strict';
import { inspect } from 'node:util';
import { Sensitive, luhn, canonical, findings, coverage, unionArea } from './metrics.mjs';

const word = (text, x) => ({ text, vertices: [[x, 0], [x + 10, 0], [x + 10, 10], [x, 10]] });
const ocr = words => ({ paragraphs: [{ lines: [{ words, vertices: [[0, 0], [100, 0], [100, 12], [0, 12]] }] }] });
test('Sensitive diagnostics hide synthetic content', () => {
  const value = new Sensitive('synthetic-only');
  assert.equal(String(value), '[redacted]');
  assert.equal(JSON.stringify(value), '"[redacted]"');
  assert.equal(inspect(value), '[redacted]');
});
test('Luhn accepts published fixtures and rejects look-alikes', () => {
  assert.ok(luhn('4242 4242 4242 4242'));
  assert.ok(luhn('4111-1111-1111-1111'));
  assert.ok(luhn('5555 5555 5555 4444'));
  assert.equal(luhn('4242 4242 4242 4241'), false);
  assert.equal(luhn('0000000000000000'), false);
});
test('split separators map back to all producing words', () => {
  const result = findings(ocr([word('qa1234', 0), word('@', 15), word('example', 30), word('.', 45), word('org', 60)]));
  assert.equal(result.length, 1);
  assert.equal(result[0].category, 'email');
  assert.equal(result[0].rects.length, 5);
  assert.ok(canonical('email', result[0].value.expose()) === 'qa1234@example.org');
});
test('benign corpus examples do not trigger detectors', () => {
  for (const text of ['user@localhost', '1.24.16', '2026-01-02T12:34:56Z', 'deadbeef01234567', '4242 4242 4242 4241']) assert.equal(findings(ocr([word(text, 0)])).length, 0);
});
test('both requested phone formats survive separator whitespace', () => {
  for (const text of ['+1 555 - 0123 - 1234', '( 555 ) 012 - 1234']) {
    const result = findings(ocr([word(text, 0)]));
    assert.equal(result.length, 1);
    assert.equal(result[0].category, 'phone');
  }
});
test('union coverage neither double-counts overlapping boxes nor hides gaps', () => {
  const glyphs = [{ x: 0, y: 0, width: 10, height: 10 }];
  const boxes = [{ x: 0, y: 0, width: 4, height: 10 }, { x: 2, y: 0, width: 4, height: 10 }];
  assert.equal(unionArea(boxes), 60);
  assert.equal(coverage(glyphs, boxes, 0), .6);
  assert.equal(coverage(glyphs, boxes, 4), 1);
});
test('fallback geometry maps back before padding and supports parent lines', () => {
  const input = ocr([word('qa1234@example.org', 0)]);
  const words = findings(input, 2), lines = findings(input, 2, 'line');
  assert.equal(words[0].rects[0].width, 5);
  assert.equal(lines[0].rects[0].width, 50);
});
test('full synthetic fixture detector contract covers every positive occurrence', () => {
  const positives = ['dev1234@example.com', 'qa1234@example.org', 'ops1234@example.net', '+1 555-0134-1234', '(555) 014-1234', '4242 4242 4242 4242', '4111-1111-1111-1111', '5555 5555 5555 4444'];
  const input = { paragraphs: [{ lines: positives.map(text => ({ words: [word(text, 0)], vertices: [[0, 0], [100, 0], [100, 12], [0, 12]] })) }] };
  const result = findings(input);
  assert.equal(result.filter(f => f.category === 'email').length, 3);
  assert.equal(result.filter(f => f.category === 'phone').length, 2);
  assert.equal(result.filter(f => f.category === 'card').length, 3);
});
test('record boundaries prevent joining unrelated numeric OCR lines into a card', () => {
  const input = { paragraphs: [{ lines: ['4242 4242', '4242 4242'].map((text, i) => ({ words: [word(text, i * 20)], vertices: [[0, 0], [100, 0], [100, 12], [0, 12]] })) }] };
  const result = findings(input);
  assert.equal(result.length, 0);
});
test('following timestamp does not swallow a valid card endpoint', () => {
  const input = ocr([word('4242 4242 4242 4242', 0), word('12:34:56 INFO', 20)]);
  const result = findings(input);
  assert.equal(result.length, 1);
  assert.equal(result[0].category, 'card');
  assert.equal(result[0].rects.length, 1);
});
