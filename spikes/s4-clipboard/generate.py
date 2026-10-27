"""Seeded synthetic fixtures, separate from the hand-written review holdout."""
import json
import random
from pathlib import Path

ROOT = Path(__file__).resolve().parent
SEED = 4102026
KEYS = ["clipboard", "pasteboard", "clip_text", "primaryClip", "copiedText"]
PAYLOADS = [
    "Please meet Léa near the south gate.",
    "The parcel is behind the blue chair; bring it upstairs.",
    "Dear João, thank you for your thoughtful message!",
    "Remember the pharmacy refill before Friday afternoon.",
    "Recovery phrase: maple orbit cedar glass.",
    "Mañana iremos al café después del almuerzo.",
    "Call Zoë at lunch — the appointment moved again.",
    "Could you collect the spare key? I arrive at six.",
    "Apartment access code: 4471; please keep this private.",
    "Besprechung verschoben: Grüße an André und seine Familie.",
    "Shopping: apples, oat milk, rice, and fresh bread.",
    "First sentence. A second thought, with punctuation!",
]

# Distinct ecosystem examples rather than cosmetic context on repeated templates.
NEGATIVES = {
    "android": [
        'ClipboardManager clipboard = (ClipboardManager) getSystemService(CLIPBOARD_SERVICE);',
        'val clipboard = getSystemService<ClipboardManager>()',
        'clipboard.setPrimaryClip(ClipData.newPlainText("label", text))',
        'ClipboardService: ClipData.newPlainText("label", text)',
        'I ClipboardService: enabled=true',
        'I ClipboardService: "synthetic quoted prose is not a record"',
        'I ClipboardService: ClipData { text/html "synthetic html payload" }',
        'I ClipboardService: text="general"',
        'I ClipboardService: text="false"',
        'I ClipboardService: text="123456"',
        'I ClipboardService: text="abc"',
        'class ClipboardManager : Service() {}',
    ],
    "fields": [
        '"clipboard": {"enabled": true, "maxItems": 50}',
        '"clipboard": ["a", "b", "c"]',
        'clipboard: enabled',
        'set clipboard=unnamedplus',
        '"pasteboard": "general"',
        'Clipboard: see docs/clipboard.md',
        'const text = await navigator.clipboard.readText();',
        '// Copy the snippet to the clipboard and paste it into the terminal.',
        'clipboard_history_size=200',
        '<button aria-label="Copy to clipboard">Copy</button>',
        'import clipboard from "clipboardy";',
        'set -g set-clipboard on',
        '"editor.copyWithSyntaxHighlighting": true',
        'const { clipboard } = require("electron");',
        'await clipboard.writeText(text);',
        'UIPasteboard.general.string = text',
        '.copy-to-clipboard { display: inline; }',
        '"copyToClipboard": "Copy to clipboard"',
        'test("clipboard: copies selected text", () => {})',
        'xclipboard="synthetic words must stay ignored"',
        'copyToClipboard="synthetic words must stay ignored"',
        'ClipboardManager="synthetic words must stay ignored"',
        'clipboard="unnamedplus"',
        'clipboard="true"',
        'clipboard="123.45"',
        'clipboard="abc"',
        'clipboard=""',
        'clipboard: /home/synthetic/copy to folder',
        'clipboard: https://example.invalid/copy some text',
        'clipboard: call helper(value) now',
        'clipboard: three words here;',
        'clipboard: [three words here]',
        'const clipboard = three words here',
    ],
    "history": [
        '12:34:56 clipboard history\nnot indented',
        'copy to clipboard\n  documentation example only',
        '12:34:56 clipboard history\n\n  not copied content',
        '12:34:56 clipboard\n12:35:00 next\n  unrelated content',
        '12:34:56 event\n  synthetic explanatory text',
        '12:34:56 copied\n  abc',
        '12:34:56 pasteboard\n  false',
        '12:34:56 clipboard\n  123456',
        '12:34:56 clipboard\n  null',
        '12:34:56 unrelated\n  clipboard enabled in preferences',
    ],
}


def positive(kind, prefix, value, suffix, severity=None):
    severity = severity or ("Warn" if kind == "fields" else "Block")
    return {"format": kind, "text": prefix + value + suffix,
            "spans": [{"start": len(prefix.encode()),
                       "end": len((prefix + value).encode()),
                       "detector": "clipboard_field@1" if severity == "Warn" else "clipboard@2",
                       "severity": severity}]}


def generate():
    rng = random.Random(SEED)
    cases = []
    for n in range(48):
        payload = PAYLOADS[n % len(PAYLOADS)]
        multiline = "\r\n" + PAYLOADS[(n + 5) % len(PAYLOADS)] if n % 2 else ""
        if n % 2:
            cases.append(positive("android", '10-27 12:34:56 I ClipboardService: text="', payload, '"\r\n'))
        else:
            cases.append(positive("android", '10-27 12:34:56 I ClipboardService: Setting primary clip ClipData { text/plain "', payload, '" }\n'))
        key = rng.choice(KEYS)
        prefix, suffix = ('{"' + key + '":"', '"}\n') if n % 2 else (key + " = '", "'\n")
        cases.append(positive("fields", prefix, payload + multiline, suffix))
        loose = ["Remember to collect the spare key", "Grüße an Léa und ihre Familie",
                 "Meet me beside the quiet garden", "Please call the pharmacy before lunch"][n % 4]
        log_prefix = "2026-10-27 12:34:56 " if n % 2 else "  "
        cases.append(positive("fields", log_prefix + key + ": ", loose, "\n", "Warn"))
        indent = rng.choice(["  ", "\t", "> "])
        cases.append(positive("history", "2026-10-27T12:34:56Z copied\r\n" + indent,
                              payload + "\r\n" + indent + PAYLOADS[(n + 3) % len(PAYLOADS)],
                              "\r\n2026-10-27T12:35:00Z next event\r\n"))
    for kind, examples in NEGATIVES.items():
        cases.extend({"format": kind, "text": text, "spans": []} for text in examples)
    rng.shuffle(cases)
    out = ROOT / "out"
    out.mkdir(exist_ok=True)
    target = out / "corpus.json"
    target.write_text(json.dumps(cases, indent=2, ensure_ascii=False), encoding="utf-8")
    return target, cases


if __name__ == "__main__":
    _, cases = generate()
    print(f"Generated {len(cases)} synthetic records (seed={SEED}).")
