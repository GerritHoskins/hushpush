// Independent construction corpus: no product modules or spike generators imported.
import fs from 'node:fs/promises';
import path from 'node:path';
import { createRequire } from 'node:module';
import { fileURLToPath } from 'node:url';
import { createHash } from 'node:crypto';

const split = process.argv.includes('--split') ? process.argv[process.argv.indexOf('--split') + 1] : 'dev';
if (!['dev', 'test'].includes(split)) throw Error('invalid split');
const root = path.join(path.dirname(fileURLToPath(import.meta.url)), ...(split === 'test' ? ['test'] : []));
await fs.mkdir(root, { recursive: true });
const seed = split === 'test' ? 20261028 : 20261027;
let state = seed;
const random = () => ((state = (Math.imul(state, 1664525) + 1013904223) >>> 0) / 2 ** 32);
const files = [];
const severities = { email: 'warn', payment_card: 'block', phone: 'warn', home_path: 'warn', private_key: 'block', clipboard_record: 'block', user_pattern: 'block' };
const emails = ['Ada.Lovelace+build@example.com', 'test-user@ci.example.org', 'UPPER.CASE@EXAMPLE.NET', 'qa+nightly@sub.service.example', 'first.last@release.test', 'dev-tools@never.invalid', 'a+b@deep.sub.example.com', 'robot.42@unit.example'];
// Published sandbox values only; provenance is in README.md.
const cards = [
  ['stripe', 'visa', '4242424242424242'], ['stripe', 'mastercard-2', '2223003122003222'],
  ['stripe', 'mastercard-51', '5105105105105100'], ['braintree', 'visa', '4111111111111111'],
  ['braintree', 'amex', '378282246310005'], ['braintree', 'discover', '6011000991300009'],
  ['adyen', 'mastercard-54', '5454545454545454'], ['adyen', 'mastercard-2', '2222400010000008'],
  ['adyen', 'amex', '371449635398431'], ['adyen', 'discover', '6011111111111117'],
  ['braintree', 'mastercard-55', '5555555555554444'],
];
const phones = [
  ['DE', '+49 30 5550 1234', '030 5550 1234'], ['GB', '+44 1632 960123', '01632 960123'],
  ['FR', '+33 1 99 00 12 34', '01 99 00 12 34'], ['NL', '+31 20 555 0123', '020 555 0123'],
  ['ES', '+34 910 00 12 34', '910 00 12 34'], ['IT', '+39 06 5550 1234', '06 5550 1234'],
  ['JP', '+81 3 5550 1234', '03-5550-1234'], ['AU', '+61 2 5550 1234', '02 5550 1234'],
  ['US', '+1 202 555 0134', '202-555-0134'], ['NANP-local', '(555) 010-2345', '555-0134'],
];
const usernames = ['ada', 'sam.dev', 'build-user', 'person42', 'josé', 'Anna Maria'];
const contexts = ['android-logcat', 'terminal-build', 'ci-job', 'crash-dump', 'agent-jsonl', 'configs', 'markdown', 'source'];
const bodies = ['The violet parcel belongs beside the west elevator.', 'A quiet lantern marks the rehearsal entrance.', 'Please leave three paper cranes beside the archive.', 'The rehearsal starts after the brass bell rings.'];
const pemTypes = ['RSA PRIVATE KEY', 'EC PRIVATE KEY', 'OPENSSH PRIVATE KEY', 'PRIVATE KEY', 'ENCRYPTED PRIVATE KEY', 'RSA PRIVATE KEY', 'PRIVATE KEY', 'EC PRIVATE KEY'];
const patternValues = ['Coralie Fable', 'Orion Wren', 'CASE-ZETA-0042'];
const patterns = 'version = 1\n[[pattern]]\nid = "coralie"\nliterals = ["Coralie Fable"]\nseverity = "block"\n[[pattern]]\nid = "orion"\nliterals = ["Orion Wren"]\nseverity = "block"\n[[pattern]]\nid = "case-zeta"\nregex = \'\\bCASE-ZETA-[0-9]{4}\\b\'\nseverity = "block"\n';
await fs.writeFile(path.join(root, 'patterns.toml'), patterns);

function encode(context, value, index) {
  if (context === 'agent-jsonl') return JSON.stringify(value).slice(1, -1);
  return value;
}
function envelope(context, index, value) {
  switch (context) {
    case 'android-logcat': return `10-27 12:34:56.789  1234  2345 I Evidence: ${value}\n`;
    case 'terminal-build': return `[step ${index}] ${value}; elapsed=0.087s\n`;
    case 'ci-job': return `2026-10-27T12:34:56Z runner INFO ${value}\n`;
    case 'crash-dump': return `    frame[${index}] debug_note: ${value}\n`;
    case 'agent-jsonl': return `{"type":"${index % 2 ? 'tool_result' : 'tool_call'}","tool":"${index % 2 ? 'exec_command' : 'Bash'}","content":"${encode(context, value, index)}"}\n`;
    case 'configs': return `${value}\n`;
    case 'markdown': return `- Evidence ${index}: \`${value}\`\n`;
    case 'source': return `// diagnostic ${index}: ${value}\n`;
    default: throw Error('unknown context');
  }
}

for (const [ci, context] of contexts.entries()) {
  const count = context === 'source' ? 10 : context === 'configs' ? 9 : 8;
  for (let i = 0; i < count; i++) {
    let text = '';
    const labels = [];
    function add(category, payload, surround = v => v, metadata = {}) {
      const { corePayload = payload, ...attributes } = metadata;
      const row = envelope(context, i, surround(payload));
      const inserted = encode(context, payload, i);
      const at = row.indexOf(inserted);
      if (at < 0) throw Error('construction span missing');
      const coreText = encode(context, corePayload, i);
      const coreAt = row.indexOf(coreText, at);
      if (coreAt < 0) throw Error('construction core missing');
      labels.push({ category, severity: severities[category], core: { start: Buffer.byteLength(text + row.slice(0, coreAt)), end: Buffer.byteLength(text + row.slice(0, coreAt) + coreText) }, extent: { start: Buffer.byteLength(text), end: Buffer.byteLength(text + row.trimEnd()) }, ...attributes });
      text += row;
    }
    const n = ci * 11 + i + (split === 'test' ? 1 : 0);
    const mail = emails[n % emails.length];
    add('email', mail, [v => `<${v}>,`, v => `"${v}".`, v => `https://portal.test/member/${v}?view=brief`, v => `owner='${v}';`][n % 4]);
    const card = cards[n % cards.length];
    const grouped = n % 3 === 0 ? card[2] : card[2].match(/.{1,4}/g).join(n % 3 === 1 ? ' ' : '-');
    add('payment_card', grouped, v => `order=ORD-QX-${100 + i} date=2026-10-27 card=[${v}], amount=EUR 19.95`, { provider: card[0], network: card[1] });
    const phone = phones[n % phones.length];
    add('phone', phone[1], v => `contact: (${v});`, { country: phone[0], format: 'international', ...(phone[0] === 'NANP-local' ? { off_spec: 'phone/national-no-country-code' } : {}) });
    add('phone', phone[2], v => `local_contact="${v}".`, { country: phone[0], format: 'national', off_spec: 'phone/national-no-country-code' });
    const nationalInternational = phones[n % 8];
    add('phone', nationalInternational[1], v => `international_national_format: ${v}`, { country: nationalInternational[0], format: 'e164-national-format' });
    const user = usernames[n % usernames.length];
    const home = [`/Users/${user}/work/report.log`, `/home/${user}/src/app`, `C:\\Users\\${user}\\work\\notes.txt`, `C:/Users/${user}/work/notes.txt`, `/mnt/c/Users/${user}/work/notes.txt`, `C:\\Users\\${user}\\`][n % 6];
    // Only the username is private; platform prefixes and file names are context.
    add('home_path', user, v => `open: ${home.replace(user, v)}`, { platform: ['macos', 'linux', 'windows-backslash', 'windows-forward', 'wsl', 'windows-space'][n % 6] });
    const type = pemTypes[n % pemTypes.length];
    const bytes = Buffer.from(Array.from({ length: 96 }, () => Math.floor(random() * 256)));
    const b64 = bytes.toString('base64').match(/.{1,64}/g).join('\n');
    let pem = `-----BEGIN ${type}-----\n${n % 8 === 5 ? 'Proc-Type: 4,ENCRYPTED\nDEK-Info: AES-128-CBC,00112233445566778899AABBCCDDEEFF\n\n' : ''}${b64}${n % 8 === 6 ? '' : `\n-----END ${type}-----`}`;
    if (n % 8 === 7) pem = pem.replaceAll('\n', '\n    ');
    add('private_key', pem, v => n % 8 === 7 ? `private_key: |\n    ${v}` : v, { format: type, truncated: n % 8 === 6 });
    const body = bodies[n % bodies.length];
    let record;
    const style = n % 6;
    if (style <= 2) {
      const stamp = ['2026-10-27T12:34:56Z', '2026-10-27 12:34:56', '10/27/2026 12:34:56'][style];
      const indent = style % 2 ? '\t' : '    ';
      record = `[${stamp}] Clipboard history entry ${i}\n${indent}${body}\n${indent}Keep the paper rehearsal schedule nearby.`;
    } else if (style === 3) record = `ClipboardService: ClipData { text/plain "${body}" }`;
    else if (style === 4) record = `{"clipboard_text":"${body}"}`;
    else record = `clipboard_content: "${body}"`;
    add('clipboard_record', record, v => v, { severity: style >= 4 ? 'warn' : 'block', format: style <= 2 ? 'history' : style === 3 ? 'logcat' : 'field', multiline: style <= 2, corePayload: style <= 2 ? record.slice(record.indexOf('\n') + 1) : body, ...(style >= 4 ? { off_spec: 'clipboard/undocumented-key' } : {}) });
    if (style >= 4) {
      const key = ['clipboard', 'pasteboard', 'clip_text', 'primaryClip', 'copiedText'][n % 5];
      add('clipboard_record', body, v => `${key}: "${v}"`, { severity: 'warn', format: 'documented-field', key });
      const historyBody = `    ${body}\n    Keep the paper rehearsal schedule nearby.`;
      add('clipboard_record', historyBody, v => `[2026-10-27T12:34:56Z] Clipboard history entry ${i}\n${v}`, { format: 'history', multiline: true });
    }
    add('user_pattern', patternValues[n % 3], v => `reviewer: <${v}>;`, { pattern_id: ['coralie', 'orion', 'case-zeta'][n % 3] });
    const positiveLines = text.split('\n').length - 1;
    const negativeTemplates = [
      'trace_id=123e4567-e89b-42d3-a456-426614174000', 'sha256=fa'.padEnd(72, 'ab'),
      'listener=192.0.2.8 upstream=2001:db8::42', 'version=1.25.0 build=v2026.10.27',
      'timestamp=2026-10-27T12:34:56.789Z', 'order=ORD-QX-314159 tracking=TRACK-ZK-092AB',
      'invalid_luhn=4242-4242-4242-4241', 'invalid_luhn=4111111111111112',
      '-----BEGIN PUBLIC KEY-----', 'U1lOVEhFVElDLVBVQkxJQy1PTkxZ', '-----END PUBLIC KEY-----',
      '-----BEGIN CERTIFICATE-----', 'U1lOVEhFVElDLUNFUlRJRklDQVRF', '-----END CERTIFICATE-----',
      'reference 555-0200 and 555-9999 are extension tokens', 'i18n.en.clipboard = "Copy to clipboard"',
      'i18n.de.paste = "Aus Zwischenablage einfügen"', 'await navigator.clipboard.readText();',
      'ClipboardManager.setPrimaryClip(ClipData.newPlainText(label, text));',
      'UIPasteboard.general.string = message', 'clipboard.writeText(result);',
      'path=$HOME/work path=%USERPROFILE%/work path=/home/runner/work',
      'user@localhost @mentions npm @scope/pkg', 'clipboard_label: "Copy item"',
      'clipboard_enabled = false', 'clipboard_count = 12', 'document.execCommand("copy")',
      'error E0123 at module.rs:42:8', 'release checksum=abcdef0123456789', 'duration=00:12:34.567',
    ];
    let negativeLines = 0;
    for (let k = 0; negativeLines < Math.max(positiveLines, 40); k++) {
      const row = envelope(context, i, `${negativeTemplates[k % negativeTemplates.length]} fixture=${i} row=${k}`);
      negativeLines += row.split('\n').length - 1;
      text += row;
    }
    const ext = context === 'source' ? ['rs', 'ts', 'py', 'kt', 'swift'][i % 5] : context === 'configs' ? ['yaml', 'toml', 'json'][i % 3] : context === 'markdown' ? 'md' : context === 'agent-jsonl' ? 'jsonl' : 'log';
    // Wrap diagnostics as valid inert source/config strings, mapping spans during encoding.
    const wrappers = {
      rs: ['const FIXTURE: &str = r###"\n', '\n"###;\n'],
      ts: ['export const fixture = String.raw`\n', '\n`;\n'],
      py: ['FIXTURE = r"""\n', '\n"""\n'],
      kt: ['val fixture = """\n', '\n"""\n'],
      swift: ['let fixture = """\n', '\n"""\n'],
      toml: ["fixture = '''\n", "\n'''\n"],
      yaml: ['fixture: |\n    ', '\n'],
      json: ['{"fixture":"', '"}\n'],
    };
    if (wrappers[ext]) {
      const [prefix, suffix] = wrappers[ext];
      const offsets = new Map([[0, Buffer.byteLength(prefix)]]);
      let originalBytes = 0;
      let encoded = prefix;
      for (const char of text) {
        encoded += ext === 'json' ? JSON.stringify(char).slice(1, -1) : ext === 'yaml' && char === '\n' ? '\n    ' : char;
        originalBytes += Buffer.byteLength(char);
        offsets.set(originalBytes, Buffer.byteLength(encoded));
      }
      for (const label of labels) for (const region of [label.core, label.extent]) { region.start = offsets.get(region.start); region.end = offsets.get(region.end); }
      text = encoded + suffix;
    }
    const rel = `text/${context}/${String(i).padStart(2, '0')}.${ext}`;
    await fs.mkdir(path.dirname(path.join(root, rel)), { recursive: true });
    await fs.writeFile(path.join(root, rel), text);
    files.push({ path: rel, context, kind: 'text', expected_coverage: 'scanned', positive_lines: positiveLines, negative_lines: negativeLines, labels });
  }
}

const require = createRequire(import.meta.url);
const pw = process.env.PLAYWRIGHT_CORE ? require(process.env.PLAYWRIGHT_CORE) : require('../spikes/s1-ocr/node_modules/playwright-core');
const browser = await pw.chromium.launch({ channel: process.env.CORPUS_BROWSER || 'msedge', headless: true });
try {
  const templates = [
    { id: 'ide-terminal-panel', bg: '#20252d', fg: '#e2e7ef', title: 'Workbench — diagnostics', header: 'EXPLORER     editor.ts                         TERMINAL  OUTPUT  PROBLEMS', prompt: '$ task inspect --fixture', extra: 'src / app / diagnostics\nCompilation completed; opening diagnostic attachment.' },
    { id: 'android-emulator-logcat', bg: '#f4f6f8', fg: '#172331', title: 'Android emulator · Pixel test device', header: 'LOGCAT    device: emulator-0001    package: sample.test', prompt: '10-27 12:34:56.789  1234 I Fixture:', extra: 'DEVICE LOGCAT\nFilter: sample.test     Level: INFO' },
    { id: 'team-chat-window', bg: '#fdf9f2', fg: '#302a38', title: 'Workshop / #release-rehearsal', header: 'CHANNELS                         #release-rehearsal   PINNED', prompt: 'Synthetic teammate · Today 12:34', extra: 'Release rehearsal\nPlease review these fictional diagnostic details:' },
    { id: 'dark-browser-console', bg: '#131517', fg: '#e8e8e8', title: 'Developer tools · console.test', header: 'Elements   Console   Sources   Network       Preserve log', prompt: '> console.table(fixture)', extra: 'CONSOLE\nSandbox diagnostic event received.' },
  ];
  for (const template of templates) for (const scale of [1, 1.25, 1.5, 2]) for (const [variant, font] of [11, 13, 16].entries()) {
    const context = await browser.newContext({ viewport: { width: 960, height: 540 }, deviceScaleFactor: scale });
    const page = await context.newPage();
    const shells = {
      'ide-terminal-panel': `<header>${template.title}</header><div class="split"><aside>EXPLORER<hr>src/<br>  app/<br>    editor.ts<br>    task.ts<br><br>OUTLINE<br>diagnostics</aside><section><div class="editor">editor.ts &nbsp; ×<hr>1 &nbsp; export function inspect() {<br>2 &nbsp; &nbsp; return sandbox.run();<br>3 &nbsp; }</div><nav>TERMINAL &nbsp; OUTPUT &nbsp; PROBLEMS &nbsp; +</nav><main></main></section></div>`,
      'android-emulator-logcat': `<header>${template.title}</header><div class="split"><aside class="device"><div class="phone-screen">12:34<br><br><br>Android<br>Sandbox app<br><br>●<br><br>◁ &nbsp; ○ &nbsp; □</div><p>emulator-0001</p></aside><section><nav>LOGCAT &nbsp; sample.test &nbsp; INFO</nav><div class="filter">⌕ Search logcat &nbsp; package:mine</div><main></main></section></div>`,
      'team-chat-window': `<div class="split chat"><aside><h3>Workshop</h3>Search<br><br>CHANNELS<br># general<br># release-rehearsal<br><br>DIRECT MESSAGES<br>○ Synthetic teammate</aside><section><header>#release-rehearsal &nbsp; ☆ &nbsp; Members</header><nav>Today &nbsp; ─────────────────────</nav><div class="message"><div class="avatar">SF</div><main></main></div><div class="compose">Message #release-rehearsal &nbsp; + &nbsp; Send</div></section></div>`,
      'dark-browser-console': `<header>console.test &nbsp; /sandbox &nbsp; ◇ &nbsp; ⋮</header><nav>${template.header}</nav><div class="filter">⊘ &nbsp; top ▾ &nbsp; Filter &nbsp; Default levels ▾ &nbsp; ⚙</div><div class="console"><main></main></div><div class="drawer">› &nbsp; Console prompt<br><hr>Issues &nbsp; No test assertions pending</div>`,
    };
    await page.setContent(`<style>*{box-sizing:border-box}body{margin:0;background:${template.bg};color:${template.fg};font:${font}px Consolas,'Courier New',monospace}header{padding:14px;border-bottom:1px solid #777}nav{padding:10px;background:#7772}main{margin:12px;border:1px solid #777;padding:14px;line-height:1.8;white-space:pre-wrap}h3{font-size:16px}.split{display:grid;grid-template-columns:180px 1fr}aside{padding:18px;background:#7772;line-height:1.8;min-height:410px}.editor{padding:12px;height:104px;line-height:1.6;background:#7771}.device{padding:20px}.phone-screen{border:8px solid #35373d;border-radius:24px;background:#e3edf3;padding:12px;text-align:center;height:300px;color:#172331}.filter{padding:10px;border-bottom:1px solid #777}.chat{grid-template-columns:190px 1fr}.chat aside{background:#44304d;color:#eee;height:492px}.message{display:flex;margin:6px 12px}.message main{flex:1;margin:8px;max-width:680px}.avatar{background:#5a7482;color:white;padding:8px;height:36px;margin-top:10px}.compose{border:1px solid #777;padding:12px;margin:0 20px}.console main{margin:0;border:0;border-bottom:1px solid #555;padding:18px}.drawer{padding:12px;color:#ccc}footer{padding:8px 18px;font-size:11px;color:${template.fg}}</style>${shells[template.id]}<footer>fixture-only · UUID 123e4567-e89b-42d3-a456-426614174000 · version 1.25.0</footer>`);
    const planted = [
      { category: 'email', value: emails[(variant + templates.indexOf(template) + (split === 'test' ? 1 : 0)) % emails.length], severity: 'warn' },
      { category: 'payment_card', value: ['4242 4242 4242 4242', '5555-5555-5555-4444', '378282246310005'][variant], severity: 'block' },
      { category: 'phone', value: ['+1 202 555 0134', '+44 1632 960123', '+49 30 5550 1234'][variant], severity: 'warn' },
    ];
    const labels = await page.evaluate(({ template, planted, scale }) => {
      const main = document.querySelector('main');
      main.append(document.createTextNode(`${template.extra}\n${template.prompt}\n`));
      const labels = [];
      for (const item of planted) {
        main.append(document.createTextNode(`${item.category}: `));
        const span = document.createElement('span');
        span.textContent = item.value;
        main.append(span, document.createTextNode('\n'));
        const boxes = [];
        // Per-character DOM ranges preserve proportional glyph advances and wrapped lines.
        for (let j = 0; j < span.firstChild.length; j++) {
          const range = document.createRange();
          range.setStart(span.firstChild, j); range.setEnd(span.firstChild, j + 1);
          for (const r of range.getClientRects()) boxes.push([r.x * scale, r.y * scale, r.width * scale, r.height * scale]);
        }
        labels.push({ category: item.category, severity: item.severity, core: { boxes }, extent: { boxes } });
      }
      main.append(document.createTextNode('request=REQ-QX-0087; duration=0.087s; endpoint=192.0.2.8\n@mentions npm @scope/pkg user@localhost'));
      return labels;
    }, { template, planted, scale });
    const rel = `images/${template.id}/${scale}x-${font}px.png`;
    await fs.mkdir(path.dirname(path.join(root, rel)), { recursive: true });
    await page.screenshot({ path: path.join(root, rel) });
    files.push({ path: rel, context: 'screenshot', kind: 'image', template: template.id, scale, font_px: font, expected_coverage: 'scanned', labels });
    await context.close();
  }
} finally { await browser.close(); }

function crc32(buf) {
  let crc = 0xffffffff;
  for (const byte of buf) { crc ^= byte; for (let b = 0; b < 8; b++) crc = (crc >>> 1) ^ (crc & 1 ? 0xedb88320 : 0); }
  return (crc ^ 0xffffffff) >>> 0;
}
const signature = Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]);
const ihdr = Buffer.alloc(25); ihdr.writeUInt32BE(13); ihdr.write('IHDR', 4); ihdr.writeUInt32BE(100000, 8); ihdr.writeUInt32BE(100000, 12); ihdr[16] = 8; ihdr[17] = 2; ihdr.writeUInt32BE(crc32(ihdr.subarray(4, 21)), 21);
const edges = [
  ['binary.bin', Buffer.from([0, 1, 2, 3, 0, 255, 128]), 'unsupported', 'binary'],
  ['large-object.dat', Buffer.from('version https://git-lfs.github.com/spec/v1\noid sha256:' + 'a'.repeat(64) + '\nsize 123456\n'), 'unsupported', 'lfs-pointer'],
  ['document.pdf', Buffer.from('%PDF-1.7\n% synthetic stub, no document content\n%%EOF\n'), 'unsupported', 'unsupported-file-type'],
  ['truncated.png', signature, 'incomplete', 'image-decode-failed'],
  ['oversized.png', Buffer.concat([signature, ihdr]), 'incomplete', 'image-too-large'],
  ['non-utf8.log', Buffer.from([0x64, 0x69, 0x61, 0x67, 0x3a, 0x20, 0xff, 0xfe, 0x0a]), 'incomplete', 'partial-decode'],
];
await fs.mkdir(path.join(root, 'edge'), { recursive: true });
for (const [name, bytes, coverage, reason] of edges) {
  await fs.writeFile(path.join(root, 'edge', name), bytes);
  files.push({ path: `edge/${name}`, kind: 'edge', context: 'edge', expected_coverage: coverage, expected_reason: reason, labels: [] });
}
let symlink = 'unavailable';
try {
  try { await fs.symlink('binary.bin', path.join(root, 'edge', 'link.log')); }
  catch (e) { if (e.code !== 'EEXIST' || await fs.readlink(path.join(root, 'edge', 'link.log')) !== 'binary.bin') throw e; }
  files.push({ path: 'edge/link.log', kind: 'edge', context: 'edge', expected_coverage: 'info', expected_reason: 'symlink', labels: [], symlink_target: 'binary.bin' });
  symlink = 'created';
} catch (e) { if (e.code !== 'EPERM') throw e; }
for (const file of files) {
  if (!file.symlink_target) file.sha256 = createHash('sha256').update(await fs.readFile(path.join(root, file.path))).digest('hex');
}
await fs.writeFile(path.join(root, 'labels.json'), JSON.stringify({ version: 2, seed, split, authoring_contract: 'specification + public README + CLI help + report schema; no detector or spike-generator reads', symlink, files }, null, 2) + '\n');
console.log(`Generated ${files.length} fixtures, ${files.filter(f => f.kind === 'image').length} PNGs; labels from construction. Symlink: ${symlink}.`);
