# LeakLens

> Working name. The project will be renamed before its first release, because "LeakLens" is already used by other projects.

**A local, offline pre-push check that finds personal data in screenshots and logs before they reach a Git remote.**

Developers commit more and more debugging evidence: screenshots, terminal output, device logs, and AI-agent transcripts. These artifacts can quietly contain emails, card numbers, or other personal details. Secret scanners such as [gitleaks](https://github.com/gitleaks/gitleaks), [Betterleaks](https://github.com/betterleaks/betterleaks) and [TruffleHog](https://github.com/trufflesecurity/trufflehog) look for credentials, but they don't read the text inside images. LeakLens is built to run **alongside** them, not to replace them.

> **Status: pre-alpha.** Text and screenshot scanning, the Git modes and redacted copies (`redact`) work. Allowlists, more detectors and release packaging are still to come. Don't rely on it yet.

## What it does

- **Scans what you are actually about to publish**, not only the working tree:
  - `scan dir <PATH>`: files on disk
  - `scan staged`: the bytes in Git's index, including the temporary index Git uses for `git commit -- <path>` and `git commit -a`
  - `scan commits A..B` and `hook pre-push`: every blob new in the outgoing commits, **including intermediate commits** whose content a later commit removed
- **Reads screenshots locally** using PP-OCRv5 (mobile) models via ONNX Runtime. Findings are reported as bounding boxes.
- **Never prints what it found.** Output is shape-only, such as `[EMAIL len=21]`. In the code, matched values are wrapped in a type that cannot be formatted.
- **Fails closed.** If something couldn't be scanned (an unsupported or oversized file, OCR unavailable, a missing object in a partial clone), the scan reports **incomplete coverage** with exit code 3. It never presents that as a clean result.
- **Makes redacted copies you review yourself.** `redact <PATH> --out <DIR>` writes copies into a separate, empty folder and never touches the originals. Text values become `[REDACTED:EMAIL]`. Screenshots get opaque black boxes and are re-encoded as PNG with all metadata (EXIF, text chunks) dropped. Every copy is rescanned, and a copy that still contains findings is deleted. Nothing is staged for you.
- **Works offline.** No account, no upload, no LLM. The only network access is an explicit, hash-verified model download.

## Detectors (current)

| Detector | Severity | Notes |
|---|---|---|
| Payment card | block | 13–19 digits, known IIN prefixes, Luhn check, tolerant of adjacent dates and IDs |
| Email | warn | reserved test domains are ignored by default |
| Phone (`phone@1`) | warn | international/NANP formats; bare IDs, cards, dates, times, IPs and versions excluded; fictional numbers ignored by default |
| Home path (`home_path@1`) | warn | username segment only in Unix, Windows and JSON-escaped home paths; placeholders/service accounts ignored |
| PEM (`pem@1`) | block | private-key blocks, including truncated pastes; public keys and certificates excluded |
| Clipboard (`clipboard@2`) | block | ClipboardService quoted ClipData/text and timestamped multiline history; synthetic [S4 results](spikes/s4-clipboard/RESULTS.md) |
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

User patterns are loaded only from `LEAKLENS_PATTERNS`, otherwise
`%APPDATA%/leaklens/patterns.toml` on Windows or
`$XDG_CONFIG_HOME/leaklens/patterns.toml` (fallback `~/.config/leaklens/patterns.toml`).
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

## Quick start (from source)

```bash
cargo build --release
cargo xtask fetch-models          # PP-OCRv5 mobile + ONNX Runtime, SHA-256 verified, into ./models
./target/release/leaklens scan dir ./evidence
./target/release/leaklens redact ./evidence --out ./evidence-redacted   # review, then git add the copies yourself
./target/release/leaklens hook print > .git/hooks/pre-push   # review it, then chmod +x
```

## Reports, exit codes / configuration

Every scan mode and `hook pre-push` accepts `--json <FILE>` to write a `leaklens.report/v1` report. `--json -` writes JSON to stdout and the human report to stderr. Otherwise the human report stays on stdout. Findings are ordered by path and location; previews are shape-only. Reports include paths, repository-local fingerprints, detector versions, coverage, policies, suppression IDs, and the scan exit code. They contain no detected values, allow reasons, or configuration paths. The contract is in [`docs/report.schema.json`](docs/report.schema.json).

Report files are written through a temporary file in the target directory and atomically replace an existing regular file. Symlink and directory targets are refused with exit 2. Unix report files are created with mode `0600`; on Windows they inherit the containing directory's ACL (the profile ACL when stored under your private profile). Choose a private directory. A report inside a Git working tree that is not Git-ignored produces one warning; add report paths to `.gitignore` because they list paths and fingerprints. Write errors produce a generic message and exit 4; the human report still prints. Successful JSON output preserves the scan exit code.

Exit codes: `0` no blocking findings or coverage policies violated ? `1` blocking findings ? `2` usage/config error ? `3` coverage violates a blocking policy ? `4` internal error. Findings take precedence: **1 > 3 > 0**. Unsupported content warns by default; failures for supported content block by default.

Every mode reads configuration from the working tree: `--config <FILE>`, otherwise `.leaklens.toml` at the Git top level of the current directory (or `-C`), otherwise defaults. Index and historical config blobs never control a scan. Every key is optional; unknown keys, invalid values, and unsupported versions are errors.

```toml
version = 1

[policy]
on_incomplete = "block" # block | warn: OCR/decode failures, limits, unreadable/missing objects
on_unsupported = "warn" # block | warn: binary, unsupported types, submodules, LFS pointers

[detectors.email]
severity = "warn" # block | warn | off
include_reserved_domains = false

[detectors.card]
severity = "block" # block | warn | off

[limits]
max_text_mib = 32
max_image_mib = 50
max_image_megapixels = 50
ocr_timeout_secs = 20
```

`--include-reserved-domains` and `--on-incomplete <block|warn>` override configuration in every scan mode. Hard caps are 256 MiB text, 200 MiB images, 200 megapixels, and 120 seconds OCR; limits must be positive integers. Disabled detectors are omitted from findings and the footer. Directory symlinks are listed, never followed, and do not violate coverage policies. LFS pointers are listed as unsupported because the real content is not scanned. A `warn` policy prints a coverage warning and permits exit 0 when there are no blocking findings. `redact` retains its stricter copy/rescan and failure exit rules regardless of coverage policy. Committed allow rules contain only path and detector scope, never detected values:

```toml
[[allow]]
id = "docs-fixtures"
paths = ["docs/fixtures/**"] # matched against the displayed report path
detectors = ["email"] # optional; omit for all detectors
reason = "Synthetic addresses used in docs"
expires = "2027-01-31" # optional YYYY-MM-DD; active through this date (UTC)
```

IDs must be unique and contain 1-40 lowercase letters, digits or hyphens. Paths require at least one valid glob; catch-all patterns (`*`, `**`, `**/*`) are rejected. A nonempty reason is required. Unknown fields and invalid dates are errors. Do not put detected values in paths or reasons.

In Git repositories, each finding includes `fp=<16 lowercase hex>`. These are the first 8 bytes of HMAC-SHA256 over `detector_id || 0x00 || normalized_value`, using a random 32-byte key for that repository. Emails are lowercased; cards retain digits only. Linked worktrees share the key and value allows through Git's common directory. Configuration and local state use the current directory (or `-C`) as their repository context; use `-C <REPO>` when scanning a different repository.

```bash
leaklens allow value <FP> --reason "Reviewed synthetic fixture" --expires 2027-01-31
leaklens allow list
leaklens scan staged --no-allow
leaklens forget --yes
```

Local state lives in `<git-common-dir>/leaklens/key` and `allow.toml`, outside tracked working-tree files. The allow file stores only fingerprints, reasons, added dates and optional expiry dates. Never put a detected value in a reason. Unix key and allow files are created with mode 0600; Windows relies on the user profile ACL, so keep the repository and its Git directory under a private account. `forget` requires `--yes`, deletes this directory and invalidates the old fingerprints. Outside Git, fingerprints and value allows are unavailable.

Suppressed findings remain masked in reports with `suppressed by allow:<id>` or `suppressed by value:<fp>` and do not affect the exit code. Footers count suppressions and list expired and unused rules. `--no-allow` disables every rule for the run. Outgoing scans print an informational notice when outgoing blobs include `.leaklens.toml`. Redaction continues to redact all detected values, including those allowed for scanning.

## Measured OCR quality (synthetic corpus)

These numbers come from 60 generated screenshots (terminal, chat, devtools and log-viewer layouts at 1x, 1.5x and 2x; fake data only):

| | 1x | 2x |
|---|---|---|
| Email recall | 93% | 100% |
| Card recall | 92% | 100% |
| False findings on look-alikes | 0 | 0 |

The worst warm time was 1.54 s per 1080p image on an 8-core desktop. Laptop performance is not measured yet. See [`xtask/RESULTS.md`](xtask/RESULTS.md) and the spikes in [`spikes/`](spikes/).

## Limitations

A run with no findings does **not** mean an artifact is safe to publish. LeakLens misses:
- unsupported formats (PDF, video)
- non-Latin scripts
- rotated, stylized or low-resolution text
- free-form personal prose (names, addresses, messages)
- values split across lines or images
- encoded content
- secrets other than those listed above, so run a secret scanner as well

## Docs

- [Implementation plan](docs/plan.md)
- [Agent and contributor rules](AGENTS.md)

## License

Dual-licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your option. The OCR models are Apache-2.0 (PaddlePaddle), and ONNX Runtime is MIT.
