# Reports, exit codes and configuration


Every scan mode and `hook pre-push` accepts `--json <FILE>` to write a `hushpush.report/v1` report. `--json -` writes JSON to stdout and the human report to stderr. Otherwise the human report stays on stdout. Findings are ordered by path and location; previews are shape-only. Reports include paths, repository-local fingerprints, detector versions, coverage, policies, suppression IDs, and the scan exit code. They contain no detected values, allow reasons, or configuration paths. The contract is in [`docs/report.schema.json`](report.schema.json).

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
