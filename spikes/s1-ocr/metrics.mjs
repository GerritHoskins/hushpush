export class Sensitive {
  #value;
  constructor(value) { this.#value = value; }
  expose() { return this.#value; }
  toString() { return '[redacted]'; }
  toJSON() { return '[redacted]'; }
  [Symbol.for('nodejs.util.inspect.custom')]() { return '[redacted]'; }
}

export function luhn(value) {
  const digits = value.replace(/\D/g, '');
  if (digits.length < 13 || digits.length > 19 || /^(\d)\1+$/.test(digits)) return false;
  let total = 0;
  for (let i = digits.length - 1, double = false; i >= 0; i--, double = !double) {
    let n = Number(digits[i]); if (double) { n *= 2; if (n > 9) n -= 9; } total += n;
  }
  return total % 10 === 0;
}
export const canonical = (category, value) => category === 'email' ? value.replace(/\s/g, '').toLowerCase() : value.replace(/\D/g, '');

export function findings(ocr, factor = 1, level = 'word') {
  const words = [], mapping = [];
  let text = '';
  for (const paragraph of ocr.paragraphs) for (const line of paragraph.lines) {
    for (const word of line.words) {
      const vertices = level === 'line' ? line.vertices : word.vertices;
      const xs = vertices.map(p => p[0] / factor), ys = vertices.map(p => p[1] / factor);
      const rect = { x: Math.min(...xs), y: Math.min(...ys), width: Math.max(...xs) - Math.min(...xs), height: Math.max(...ys) - Math.min(...ys) };
      const index = words.push(rect) - 1;
      const value = word.text.replace(/\s+/g, ' ');
      for (const char of value) { text += char; mapping.push(index); }
      text += ' '; mapping.push(-1);
    }
    text += '\n'; mapping.push(-1);
  }
  const patterns = {
    email: /[a-zA-Z0-9_%+.-]+\s*@\s*[a-zA-Z0-9-]+(?:\s*\.\s*[a-zA-Z0-9-]+)+/g,
    phone: /(?:\+\s*1\s*555\s*[- ]\s*01\d{2}\s*[- ]\s*\d{4}|\(\s*555\s*\)\s*01\d\s*[- ]\s*\d{4})/g,
    card: /\b(?:\d[ -]*){12,18}\d\b/g,
  };
  const result = [];
  for (const [category, pattern] of Object.entries(patterns)) for (const candidate of text.matchAll(pattern)) {
    let match = candidate;
    if (category === 'card') {
      // A following timestamp can extend a numeric run. Check legal endpoints
      // before rejecting the candidate, without using fixture-specific lengths.
      let endpoint = candidate[0].length;
      while (endpoint > 0) {
        const prefix = candidate[0].slice(0, endpoint);
        if (/\d$/.test(prefix) && !/\d/.test(candidate[0][endpoint] ?? '') && luhn(prefix)) break;
        endpoint--;
      }
      if (!endpoint) continue;
      match = { 0: candidate[0].slice(0, endpoint), index: candidate.index };
    }
    const value = new Sensitive(match[0]);
    const indices = [...new Set(mapping.slice(match.index, match.index + match[0].length).filter(i => i >= 0))];
    result.push({ category, value, rects: indices.map(i => words[i]) });
  }
  return result;
}

export function unionArea(rects) {
  const xs = [...new Set(rects.flatMap(r => [r.x, r.x + r.width]))].sort((a, b) => a - b);
  let area = 0;
  for (let i = 1; i < xs.length; i++) {
    const left = xs[i - 1], right = xs[i];
    const intervals = rects.filter(r => r.x < right && r.x + r.width > left).map(r => [r.y, r.y + r.height]).sort((a, b) => a[0] - b[0]);
    let length = 0, end = -Infinity;
    for (const [a, b] of intervals) { length += Math.max(0, b - Math.max(a, end)); end = Math.max(end, b); }
    area += (right - left) * length;
  }
  return area;
}

export function coverage(glyphs, rects, padding) {
  const intersections = [];
  for (const g of glyphs) for (const r of rects) {
    const x = Math.max(g.x, r.x - padding), y = Math.max(g.y, r.y - padding);
    const right = Math.min(g.x + g.width, r.x + r.width + padding), bottom = Math.min(g.y + g.height, r.y + r.height + padding);
    if (right > x && bottom > y) intersections.push({ x, y, width: right - x, height: bottom - y });
  }
  return unionArea(intersections) / unionArea(glyphs);
}
