# Detector reference


| Detector | Severity | Notes |
|---|---|---|
| Payment card | block | 13–19 digits, known IIN prefixes, Luhn check, tolerant of adjacent dates and IDs |
| Email | warn | reserved test domains are ignored by default |
| Phone (`phone@1`) | warn | international/NANP formats; bare IDs, cards, dates, times, IPs and versions excluded; fictional numbers ignored by default |
| Home path (`home_path@1`) | warn | Unicode letters/marks, digits, `.`, `_`, `-` in the username segment only, including mounted and JSON-escaped home paths; Windows backslash paths also allow single internal spaces in segments up to 64 characters; placeholders/service accounts ignored |
| PEM (`pem@1`) | block | private-key blocks, including truncated pastes and JSON-string `\n` / `\r\n` escapes; public keys and certificates excluded |
| Clipboard (`clipboard@2`) | block | ClipboardService quoted ClipData/text and timestamped multiline history, including recognized log prefixes and JSON strings; synthetic [S4 results](../spikes/s4-clipboard/RESULTS.md) |
| Clipboard fields (`clipboard_field@1`) | warn | quoted fields and line-leading (or recognized log-prefix) unquoted prose fields; identifiers, clipboard UI text, code and scalar values excluded |
| User patterns (`user:<id>@1`) | block or warn | external literals (case-insensitive, whole-word) and/or Rust regex; severity from the pattern file |

`--include-reserved` includes both reserved email domains and fictional phone numbers.
The previous `--include-reserved-domains` spelling remains a hidden alias.
`[detectors.phone] include_reserved = true` and the existing
`[detectors.email] include_reserved_domains = true` enable them separately.
Each built-in accepts `severity = "block" | "warn" | "off"` under
`[detectors.<id>]` (`email`, `card`, `phone`, `home_path`, `pem`, `clipboard`, `clipboard_field`).
`clipboard_loose` is a deprecated configuration alias for `clipboard_field`; the CLI prints a one-line note when used. Do not configure both names.
Values containing whole-word `clipboard`, `pasteboard`, `copied`, `copy`, `paste`,
or `cut` (case-insensitive), and whitespace-free ASCII identifiers using
`A-Z a-z 0-9 _ . : / -` of any length are excluded.
Unquoted prose needs at least three words and has trailing ` #...` and ` //...` comments stripped before evaluation.
`[detectors.user_pattern]` overrides external patterns together;
`[detectors."user:family-names"]` overrides one pattern by ID.
Phone fingerprints use digits only, home/user literals lowercase, PEM removes
whitespace, and clipboard content stays unchanged. Clipboard field fingerprints now use
`clipboard_field` in the HMAC, so previous field fingerprints and value allows must be regenerated. Redaction replaces entire
multiline spans and preserves surrounding CRLF bytes.

User patterns are loaded only from `HUSHPUSH_PATTERNS`, otherwise
`%APPDATA%/hushpush/patterns.toml` on Windows or
`$XDG_CONFIG_HOME/hushpush/patterns.toml` (fallback `~/.config/hushpush/patterns.toml`).
The file must be outside the active repository. `[[pattern]]` in repository
configuration is rejected (exit 2). `--no-user-patterns` disables loading.
Unix warns once for group/world-readable files. Reports list IDs, never contents.

```toml
version = 1
[[pattern]]
id = "family-names" # unique [a-z0-9-], 1-40 characters
literals = ["Jane Doe"]
regex = '(?i)\bproject-falcon\b' # optional; compiled regex limit 1 MiB
severity = "block" # or "warn"
```
