# hushpush

> Renamed to hushpush on 2026-10-27 (formerly LeakLens; name collision).

**A local, offline pre-push check that finds personal data in screenshots and logs before they reach a Git remote.**

Developers commit more and more debugging evidence: screenshots, terminal output, device logs, and AI-agent transcripts. These artifacts can quietly contain emails, card numbers, or other personal details. Secret scanners such as [gitleaks](https://github.com/gitleaks/gitleaks), [Betterleaks](https://github.com/betterleaks/betterleaks) and [TruffleHog](https://github.com/trufflesecurity/trufflehog) look for credentials, but they don't read the text inside images. hushpush is built to run **alongside** them, not to replace them.

> **Status: pre-alpha.** Text and screenshot scanning, the Git modes and redacted copies (`redact`) work. Release acceptance and further detectors remain in progress. Don't rely on it yet.

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
| Home path (`home_path@1`) | warn | Unicode letters/marks, digits, `.`, `_`, `-` in the username segment only, including mounted and JSON-escaped home paths; Windows backslash paths also allow single internal spaces in segments up to 64 characters; placeholders/service accounts ignored |
| PEM (`pem@1`) | block | private-key blocks, including truncated pastes and JSON-string `\n` / `\r\n` escapes; public keys and certificates excluded |
| Clipboard (`clipboard@2`) | block | ClipboardService quoted ClipData/text and timestamped multiline history, including recognized log prefixes and JSON strings; synthetic [S4 results](spikes/s4-clipboard/RESULTS.md) |
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

## Install and quick start

Release packaging is configured; no release has been published yet. See [packaging evidence](docs/packaging.md).

Windows requires Windows 10 1903 or later and the [Microsoft Visual C++ Redistributable 2015–2022 (x64)](https://aka.ms/vs/17/release/vc_redist.x64.exe), which most systems already have. Without it, Windows refuses to start `hushpush.exe` with a missing `MSVCP140.dll` or `VCRUNTIME140.dll` error. Linux release binaries need glibc 2.35 or newer.

After an approved release is published, install one binary using:

```bash
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/GerritHoskins/hushpush/releases/latest/download/hushpush-cli-installer.sh | sh
# Or: brew install GerritHoskins/tap/hushpush
# Or: cargo binstall hushpush-cli
hushpush --version
hushpush scan dir ./demo/evidence --include-reserved --no-user-patterns
hushpush redact ./evidence --out ./evidence-redacted
```

Windows PowerShell installer:

```powershell
irm https://github.com/GerritHoskins/hushpush/releases/latest/download/hushpush-cli-installer.ps1 | iex
```

Review installer scripts before running them. Release builds embed the pinned OCR models and statically link ONNX Runtime; scanning performs no downloads. Synthetic demo email domains require `--include-reserved`. Exit 1 is expected because the demo contains a blocking card. `--version` reports whether models are embedded and their short hashes.

From source (Rust stable, edition 2024):

```bash
cargo xtask fetch-models          # SHA-256 verified models only
cargo build --release --locked -p hushpush-cli --features bundled-models
./target/release/hushpush scan dir ./demo/evidence --include-reserved --no-user-patterns
```

`cargo install hushpush-cli` and default-feature builds do not embed models. Run `cargo xtask fetch-models` from this checkout, then pass `--models DIR` or set `HUSHPUSH_MODELS`. Model lookup is `--models` > `HUSHPUSH_MODELS` > embedded models > models beside the canonicalized executable > debug checkout models. Explicit paths override embedded assets even if invalid. Build-time `HUSHPUSH_MODELS_DIR` changes the source directory for embedding; missing or modified files fail the build. Source builds link a prebuilt static ONNX Runtime and need a current C++ toolchain: on Windows, Visual Studio 2022 17.14 or newer (MSVC 14.44+); older MSVC fails to link with unresolved `__std_*` symbols.

Optional native pre-push hook (review before installing yourself):

```bash
hushpush hook print > .git/hooks/pre-push
# chmod +x .git/hooks/pre-push on Unix
```

With the [pre-commit](https://pre-commit.com) framework (needs pre-commit 3.2+ and `hushpush` on PATH; the hooks run the installed binary):

```yaml
repos:
  - repo: https://github.com/GerritHoskins/hushpush
    rev: v0.1.0
    hooks:
      - id: hushpush            # scans the staged index at commit time
      - id: hushpush-pre-push   # scans the blobs a push would send
```

Then run `pre-commit install --hook-type pre-commit --hook-type pre-push`. The pre-push hook validates all six `PRE_COMMIT_*` variables and exits 2 if any is missing or invalid; it never reads stdin or fetches missing Git objects. pre-commit [passes only the first eligible ref](https://github.com/pre-commit/pre-commit/blob/main/pre_commit/commands/hook_impl.py) of a multi-ref push and can omit FROM/TO for whole-history pushes, so it cannot promise complete coverage; the native hook above scans every ref.

## Reports, exit codes / configuration

Every scan mode and `hook pre-push` accepts `--json <FILE>` to write a `hushpush.report/v1` report. `--json -` writes JSON to stdout and the human report to stderr. Otherwise the human report stays on stdout. Findings are ordered by path and location; previews are shape-only. Reports include paths, repository-local fingerprints, detector versions, coverage, policies, suppression IDs, and the scan exit code. They contain no detected values, allow reasons, or configuration paths. The contract is in [`docs/report.schema.json`](docs/report.schema.json).

Report files are written through a temporary file in the target directory and atomically replace an existing regular file. Symlink and directory targets are refused with exit 2. Unix report files are created with mode `0600`; on Windows they inherit the containing directory's ACL (the profile ACL when stored under your private profile). Choose a private directory. A report inside a Git working tree that is not Git-ignored produces one warning; add report paths to `.gitignore` because they list paths and fingerprints. Write errors produce a generic message and exit 4; the human report still prints. Successful JSON output preserves the scan exit code.

Exit codes: `0` no blocking findings or coverage policies violated ? `1` blocking findings ? `2` usage/config error ? `3` coverage violates a blocking policy ? `4` internal error. Findings take precedence: **1 > 3 > 0**. Unsupported content warns by default; failures for supported content block by default.

Every mode reads configuration from the working tree: `--config <FILE>`, otherwise `.hushpush.toml` at the Git top level of the current directory (or `-C`), otherwise defaults. If only `.leaklens.toml` exists, the command exits 2 with `found .leaklens.toml; rename it to .hushpush.toml` without reading it. Index and historical config blobs never control a scan. Every key is optional; unknown keys, invalid values, and unsupported versions are errors.

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
hushpush allow value <FP> --reason "Reviewed synthetic fixture" --expires 2027-01-31
hushpush allow list
hushpush scan staged --no-allow
hushpush forget --yes
```

Local state lives in `<git-common-dir>/hushpush/key` and `allow.toml`, outside tracked working-tree files. The allow file stores only fingerprints, reasons, added dates and optional expiry dates. Never put a detected value in a reason. Unix key and allow files are created with mode 0600; Windows relies on the user profile ACL, so keep the repository and its Git directory under a private account. `forget` requires `--yes`, deletes this directory and invalidates the old fingerprints. Outside Git, fingerprints and value allows are unavailable.

Suppressed findings remain masked in reports with `suppressed by allow:<id>` or `suppressed by value:<fp>` and do not affect the exit code. Footers count suppressions and list expired and unused rules. `--no-allow` disables every rule for the run. Outgoing scans print an informational notice when outgoing blobs include `.hushpush.toml`. Redaction continues to redact all detected values, including those allowed for scanning.

## Independent corpus evaluation

The seeded, synthetic [`corpus/`](corpus/) is authored from the public contract independently of detector implementations. Its labels record planted byte spans and browser glyph boxes; screenshot PNGs and truth are checked in, so evaluation needs no browser. See the generated [`corpus/RESULTS.md`](corpus/RESULTS.md) for measured precision, recall, OCR localization, phone recall at 1x/2x, coverage, redaction and performance gates. These measurements describe this corpus, not real-world accuracy.

Run from the repository root with verified local OCR assets:

```bash
cargo xtask fetch-models
cargo build --release --locked
HUSHPUSH_MODELS="$PWD/models" cargo xtask eval
```

The harness runs the release CLI with reserved synthetic values included and the corpus user patterns loaded. It writes `target/eval.json` and regenerates `corpus/RESULTS.md`. Inputs are checked for changes during the redaction evaluation; the 50 MB throughput fixture is generated temporarily.

When `HUSHPUSH_MODELS` is absent, a set `LEAKLENS_MODELS` is ignored with a one-line hint to use `HUSHPUSH_MODELS`.

To deliberately record a reviewed baseline, run `cargo xtask eval --split dev --write-baseline` with `HUSHPUSH_MODELS` set. This generates `corpus/baseline.json`; ordinary evaluations do not update it. CI runs `cargo xtask eval --split dev --ci` and fails on a metric falling more than one percentage point below that baseline. Absolute gates remain visible as PASS/FAIL, but known gate failures alone do not fail CI. Review failures as detector evidence; do not change the corpus to make a gate pass.

The documented clipboard field keys are `clipboard`, `pasteboard`, `clip_text`,
`primaryClip`, and `copiedText`.

Labels separate contract `core` bytes from whole-record `extent` diagnostics.
Text matching requires at least 90% core coverage; spill is reported separately.
Off-spec national phones without country codes and undocumented clipboard keys
are tracked separately and excluded from gates. The sealed `corpus/test/` split
(seed `20261028`) is for milestone checks only: `cargo xtask eval --split test`.
Its manifest hash is pinned in the dev baseline; CI and baseline writes use dev
only. Test reports preserve the dev reports.

Earlier detector-adjacent measurements remain in [`xtask/RESULTS.md`](xtask/RESULTS.md) and [`spikes/s4-clipboard/RESULTS.md`](spikes/s4-clipboard/RESULTS.md).

## Limitations

OCR regions on the same visual row are joined for detection while preserving their character boxes. Image redaction makes up to three additional masking passes when changed OCR segmentation reveals another finding; the saved copy must still pass a final rescan. Persistent findings or failed rescans prevent keeping the copy.

A run with no findings does **not** mean an artifact is safe to publish. hushpush misses:
- national phone numbers without a country code
- clipboard field keys other than the documented keys
- unsupported formats: PDF (`pdf`), archives (`archive`), MP4/MOV (`video`), and GIF/WebP/BMP/TIFF/HEIC/AVIF (`image-format-unsupported`)
- non-Latin text in images (text home-path usernames accept Unicode letters and marks)
- rotated, stylized or low-resolution text
- free-form personal prose (names, addresses, messages)
- values split across physical lines or images
- encoded content beyond the supported JSON string escapes
- secrets other than those listed above, so run a secret scanner as well

### Security boundaries

PNG and JPEG dimensions are checked before decoding against the configured pixel budget; oversized images are incomplete (`image-too-large`). Decode failures are incomplete (`image-decode-failed`). Supported JPEG EXIF orientation is applied before OCR and redaction, and copies are PNG pixels without original metadata. Empty text is scanned; invalid UTF-8 is scanned lossily with partial coverage.

Directory scans list symlinks without following them, including loops and links outside the scan root. Nested `.git` files and directories are skipped while files in submodule and linked worktrees are scanned. Git scans classify LFS pointer blobs as unsupported; the external LFS object is never fetched. Printed paths and previews neutralize terminal, bidi and zero-width controls.

Redaction refuses symlink output paths and ancestors, checks again immediately before publishing the verified temporary copy, and refuses overwrite. If a matched value remains in a replacement marker or surrounding text, or substitutions reveal new findings, text redaction conservatively masks the whole copy (empty when even the opaque marker matches); review every copy for lost context. These checks assume a trusted local workspace: a hostile process concurrently changing directories can still race a path-based filesystem operation. Redaction is not a sandbox against other processes with write access to the output tree.

Decoder, parser and text-redaction fuzz targets run in a digest-pinned Docker nightly environment; see [fuzz instructions](fuzz/README.md). A short smoke run or a clean rescan does not establish real-world detection accuracy or guarantee safe publication.

## Docs

- [Implementation plan](docs/plan.md)
- [Agent and contributor rules](AGENTS.md)

## License

Dual-licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your option. The OCR models are Apache-2.0 (PaddlePaddle), and ONNX Runtime is MIT.
