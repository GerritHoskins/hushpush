import { Sensitive, luhn } from '../s1-ocr/metrics.mjs';
export { Sensitive, canonical, coverage } from '../s1-ocr/metrics.mjs';

export function rect(vertices) {
  if (!Array.isArray(vertices) || vertices.length !== 4 || vertices.some(p => p.length !== 2 || p.some(n => !Number.isFinite(n)))) throw new Error('Invalid OCR polygon');
  const xs = vertices.map(p => p[0]), ys = vertices.map(p => p[1]);
  return { x: Math.min(...xs), y: Math.min(...ys), width: Math.max(...xs) - Math.min(...xs), height: Math.max(...ys) - Math.min(...ys) };
}

export function findings(ocr, level = 'char') {
  const boxes = [], mapping = [];
  let text = '';
  for (const paragraph of ocr.paragraphs) for (const line of paragraph.lines) {
    // Keep detector text identical between localization modes. Never insert
    // separators between CTC characters, which would alter email recognition.
    const chars = line.chars ?? [];
    const lineIndex = boxes.push(rect(line.vertices)) - 1;
    const offsets = Array(line.text.length).fill(-1);
    let cursor = 0;
    for (const char of chars) {
      const start = char.start ?? cursor, end = char.end ?? start + char.text.length;
      if (start < 0 || end > line.text.length || end <= start || line.text.slice(start, end) !== char.text) throw new Error('CTC character offsets disagree with line text');
      const index = boxes.push(rect(char.vertices)) - 1;
      offsets.fill(index, start, end);
      cursor = end;
    }
    if (level === 'char' && Array.from({ length: line.text.length }, (_, i) => i).some(i => !/\s/.test(line.text[i]) && offsets[i] < 0)) throw new Error('Missing character geometry');
    // S1 collapses whitespace inside words, while retaining newlines between
    // lines. Apply the same normalization with UTF-16 offset preservation.
    for (let i = 0; i < line.text.length; i++) {
      const char = line.text[i];
      if (/\s/.test(char)) {
        if (i > 0 && /\s/.test(line.text[i - 1])) continue;
        text += ' ';
      } else text += char;
      mapping.push(level === 'line' ? lineIndex : offsets[i]);
    }
    text += ' \n'; mapping.push(-1, -1);
  }
  const patterns = {
    email: /[a-zA-Z0-9_%+.-]+\s*@\s*[a-zA-Z0-9-]+(?:\s*\.\s*[a-zA-Z0-9-]+)+/g,
    phone: /(?:\+\s*1\s*555\s*[- ]\s*01\d{2}\s*[- ]\s*\d{4}|\(\s*555\s*\)\s*01\d\s*[- ]\s*\d{4})/g,
    card: /\b(?:\d[ -]*){12,18}\d\b/g,
  };
  const result = [];
  for (const [category, pattern] of Object.entries(patterns)) for (const candidate of text.matchAll(pattern)) {
    let length = candidate[0].length;
    if (category === 'card') {
      while (length > 0) {
        const prefix = candidate[0].slice(0, length);
        if (/\d$/.test(prefix) && !/\d/.test(candidate[0][length] ?? '') && luhn(prefix)) break;
        length--;
      }
      if (!length) continue;
    }
    const indices = [...new Set(mapping.slice(candidate.index, candidate.index + length).filter(i => i >= 0))];
    result.push({ category, value: new Sensitive(candidate[0].slice(0, length)), rects: indices.map(i => boxes[i]) });
  }
  return result;
}
