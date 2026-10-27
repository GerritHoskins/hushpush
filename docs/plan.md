# LeakLens — implementation-ready plan

_Planned 2026-10-26 from `01-leaklens-planning-handoff.md`. Interview decisions: community adoption goal; Rust; OCR in the first demo; no fixed time budget; complements secret scanners._

## 1. Recommendation — narrow, then proceed

**Narrow** to: *a pre-push privacy check for screenshots and logs*, positioned as a companion to gitleaks/Betterleaks (which own secrets). **Rename before launch**: "LeakLens" already names a VS Code secret-scanning extension and the LeakLens.org breach registry.

- **Strongest evidence:** no surveyed tool combines Git-object-aware outgoing scans, OCR of screenshots, PII/evidence structures, and irreversible redacted copies in one offline binary. Neighbors each cover a slice: Betterleaks/gitleaks/TruffleHog (secrets, Git-aware, no OCR/PII), piilint (text PII, staged hook, redacted copies, no OCR), Presidio image redactor (OCR+PII library, alpha, needs Tesseract+spaCy, no Git).
- **Largest uncertainty:** whether `ocrs` (Latin-only, "early preview") localizes small/dark-mode screenshot text precisely enough for boxes to fully cover PII. Spike S1 decides; second uncertainty is alert fatigue from PII warnings.

## 2. Product contract

- **Users:** developer preparing a push (primary); maintainer checking contributed evidence via `scan commits base..head` (secondary).
- **Promise:** "Shows which outgoing logs and screenshots contain likely personal data — without printing it — and makes redacted copies you review and stage yourself." Never "safe to publish".
- **MVP:** single Rust binary; UTF-8 text/logs; PNG/JPEG with local OCR; modes `scan dir|staged|push|commits`; `hook pre-push`; masked terminal + JSON report; `redact` to a separate directory; scoped allowlists; coverage accounting.
- **Non-goals (v0.1):** broad secret detection (recommend Betterleaks), PDF/video, non-Latin OCR, history rewriting, auto-staging, hosted service, org policy, LLM dependency, network access during scans.
- **Success targets (unmeasured targets):** first result ≤60 s after prerequisites; 20 s demo; 3 outside testers complete quickstart unaided; per-category targets in §4/§6; adoption signals: external issues/PRs, pre-commit hook usage — tracked, not promised.

## 3. Architecture

**Components (one Cargo workspace):**
1. `cli` (clap) — commands, exit codes, config loading.
2. `source` — yields `(blob_oid?, path, mode, bytes)` from dir walk or Git via `git` subprocesses (`rev-list`, `cat-file --batch`). Chosen over gitoxide, whose read crates are "usable", not production-grade; `git` is always present in hook context and handles alternates/shallow/partial clones correctly.
3. `classify` — by magic bytes: PNG/JPEG → image; NUL in first 8 KB → binary/unsupported; LFS pointer → incomplete; mode 160000 (submodule) → skipped; mode 120000 (symlink) → link text scanned, never followed.
4. `extract` — text: UTF-8 (BOM stripped; invalid bytes → lossy + partial-coverage flag), records split on lines plus multiline grouping (timestamp/indent continuation). Image: decode with `image::Limits` → `ocrs` words+boxes → text with char→box map. `OcrEngine` trait so PP-OCR via `ort` can replace ocrs.
5. `detect` — rule detectors over extracted text with spans; scanned content is inert data (no templating, execution, path resolution, or agent-instruction handling).
6. `report` — findings + coverage → terminal/JSON.
7. `redact` — text: replace span with `[REDACTED:<category>]` on character boundaries, whole multiline record for clipboard findings; image: solid fill of padded boxes in the decoded pixel buffer, re-encode from raw pixels (drops EXIF/XMP/text chunks), PNG output; mandatory rescan of output.

**Data flow:** source → classify → extract → detect → allowlist filter (suppressions recorded) → report → optional redact → rescan.

**Git scan semantics:**
| Mode | Bytes scanned |
|---|---|
| `scan dir P` | working-tree files; no Git |
| `scan staged` | index blobs from `git diff --cached --raw -z --no-renames` (not working tree) |
| `scan commits A..B` | all blobs new in that range |
| `scan push` / `hook pre-push <remote> <url>` | per stdin line `<lref> <loid> <rref> <roid>`: deletion (`loid`=0) → skip; existing remote ref → `rev-list --objects --missing=print <loid> --not <roid> --remotes=<remote>` (one `--not` negates every following ref); new ref (`roid`=0) or `roid` absent locally → `--not --remotes=<remote>` only (over-scan, marked approximate); no remote-tracking refs → full history of `loid` with warning. Union and dedupe blobs across refs. Intermediate-commit blobs are included because `--objects` walks every new commit. Missing objects (partial clone) → incomplete; never lazy-fetch. |

**Severity defaults (configurable):** block — payment card (Luhn), PEM private-key block, clipboard-history record, user patterns; warn — email (RFC 2606 reserved domains benign), phone, home-dir paths with usernames.

**Allowlists:** committed `.leaklens.toml` holds only path/detector-scoped rules with required `reason`, optional `expires` — never values. Value-level allows live in local `.git/leaklens/allow.toml` as HMAC-SHA256 fingerprints keyed by a local 0600 key (low-entropy values like phones are brute-forceable from plain hashes). User patterns (own name, family names) live in user-global config outside the repo. Every suppression appears in the report with rule ID; `--no-allow` shows all.

**Exit codes:** 0 no blocking findings; 1 blocking findings; 2 usage/config error; 3 incomplete coverage under `block` policy; 4 internal error. Any non-zero aborts a push (fail closed); bypass is Git's own `--no-verify`.

**Finding schema (JSON v1):** `{fingerprint, detector, detector_version, category, severity, confidence, source:{mode, path, blob_oid, commit?}, location:{line, col_start, col_end} | {bbox:[x,y,w,h]}, preview, suppressed_by?}` plus report-level `coverage:{scanned, skipped:[{path, reason}], incomplete:bool, approximate:bool}`. Default preview is shape-only (`[EMAIL len=21]`); `--preview=partial` opt-in.

**Disclosure channels:** sensitive strings wrapped in `Sensitive<T>` whose `Debug`/`Display` print `[redacted]`; custom panic hook emits no payloads; printed paths/previews stripped of control/ANSI sequences; reports written only with explicit `--json FILE`, mode 0600 (Windows: user-profile ACL); no scan cache; no temp files (in-memory decode); outputs only under `--out`, refuse overwrite, reject `..`/absolute/reserved names; state limited to `.git/leaklens/` (delete via `leaklens forget`).

**Limits:** text 32 MB, image 50 MB / 50 MP, OCR 20 s per image (defaults) → incomplete.

**Dependencies:** clap, regex, image (png/jpeg), ocrs+rten, serde/serde_json, toml, hmac+sha2, globset (all MIT/Apache per crates; verify in T1); `git` on PATH for Git modes; OCR models shipped with releases (no runtime download; explicit `models fetch` with pinned SHA-256 as fallback).

## 4. Risk-first spikes (synthetic data only)

| ID | Hypothesis | Procedure | Pass | Fallback |
|---|---|---|---|---|
| S1 | ocrs finds and localizes planted PII in screenshots | Render 60 screenshots (terminal light/dark, chat, devtools, log viewer; 1x/1.5x/2x; 11–16 px) with fake PII and ground-truth glyph boxes; run `ocrs --json`; score | recall ≥0.85 @1x, ≥0.95 @2x; ≥98% of planted glyph area covered with ≤8 px padding; ≤1.5 s per 1080p on 4-core laptop CPU | upscale/invert preprocessing → PP-OCR ONNX via `ort` → line-level redaction boxes |
| S2 | `rev-list` recipe yields exact outgoing blob sets without fetching | Script repos: secret only in intermediate commit; new branch; multi-ref push; deletion; force-push with unknown `roid`; `--filter=blob:none` clone | exact expected sets; no fetch (GIT_TRACE); <300 ms for 100 text commits | over-scan + `approximate` flag |
| S3 | Models ship offline and redistributably | Check rten byte-loading, archive size, model weight/training-data license | runs with network disabled; license permits redistribution; archive ≤60 MB | separate model archive; switch engine |
| S4 | Clipboard records detectable structurally | Synthetic logcat/agent/clipboard-manager logs + benign "clipboard" code/docs | precision ≥0.90, recall ≥0.90 on multiline records | ship known formats only; downgrade to warn |
| S5 | Redaction is irreversible | Redact corpus; parse outputs for metadata chunks; check region pixels equal fill; rescan | 100%, no exceptions | none — must pass |

## 5. Ordered delivery backlog

| ID | Outcome | Deps | Acceptance / validation | Size |
|---|---|---|---|---|
| T1 | Workspace, `Sensitive<T>`, exit codes, CI (fmt, clippy, tests on macOS/Windows/Linux), license audit | — | CI green on 3 OSes; `cargo deny` passes | S |
| T2 | S1 harness + synthetic screenshot generator | — | reproducible corpus; S1 verdict recorded | M |
| T3 | `scan dir` text: email + card detectors, masked terminal report | T1 | fixtures pass; snapshot test proves no raw values in output | S |
| T4 | Image pipeline: limits → OCR → box map → findings with bbox | T2,T3 | demo screenshot finding reported with bbox | M |
| T5 | `redact` text+image to `--out`, rescan | T4 | S5 passes | M |
| **M1** | **Demo: log + screenshot → masked findings → clean copies** | T1–T5 | 20 s recording works on fake repo | — |
| T6 | `scan staged` from index blobs | T3 | staged-only secret found while working tree clean | S |
| T7 | `scan push`, `hook pre-push`, `scan commits` | T6 | S2 scenarios pass | L |
| T8 | Coverage model, policies, exit codes | T4,T7 | OCR-failure and missing-object scenarios exit 3 | M |
| T9 | Allowlists + audit trail | T8 | valid allow suppresses and is reported; expired allow ignored | M |
| T10 | Phone, home paths, PEM, clipboard (S4), user patterns | T3 | per-detector fixtures; S4 passes | L |
| T11 | Labeled corpus + eval harness in CI | T4,T10 | per-category P/R and OCR localization reported each run | M |
| T12 | JSON schema v1, 0600 files, report-leak test | T8 | report grep for every planted value finds nothing | S |
| T13 | Hardening: symlinks, LFS, submodules, binary, huge/malformed images, control chars, `cargo-fuzz` on decoders | T8 | each case reported as skipped/incomplete; 1 h fuzz without crash | M |
| T14 | Packaging: cargo-dist releases, bundled models, Homebrew tap, `.pre-commit-hooks.yaml` (pre-commit + pre-push stages) | T5,S3 | fresh-VM install → first result ≤60 s on 3 OSes | M |
| T15 | README quickstart, limitations, CONTRIBUTING, VHS demo | T14 | 3 outside testers succeed unaided | S |

**Cut lines:** M1 = first demo; T6–T9+T12 = usable hook (alpha); all = v0.1.

## 6. Release plan

- **Packaging:** prebuilt binaries (macOS x64/arm64, Windows x64, Linux x64/arm64) via GitHub Releases, Homebrew tap, `cargo install`/`cargo binstall`, pre-commit hook.
- **License:** MIT OR Apache-2.0 (Rust norm, matches ocrs/rten/image); third-party model terms in `NOTICE`.
- **Quickstart:** install → `scan dir ./evidence` → `redact ./evidence --out ./evidence-redacted` → review → `git add` yourself → optional pre-push hook printed by `hook print`, never auto-installed.
- **Sample corpus:** `corpus/` generated by seeded scripts; fake values only (example.com emails, 555-01xx phones, published test card numbers).
- **Demo:** `demo/` script builds a tiny repo with a device log and a screenshot; VHS tape: `scan push` → 2 masked findings → `redact` → rescan.
- **Release gates (targets):** all §7 scenarios pass; per-category precision ≥0.90 (block) / ≥0.80 (warn); recall ≥0.95 text, ≥0.85 OCR @1x; S5 100%; text 50 MB <2 s; pre-push overhead <300 ms without images.
- **Contribution entry points:** detector template (trait + fixtures + corpus labels), clipboard-format recognizers, corpus samples, alternate `OcrEngine` backends.
- **Support limitations / false negatives:** misses unsupported formats, non-Latin scripts, rotated/stylized/low-resolution text, prose PII (names, addresses, messages) unless user patterns match, values split across lines/images, encoded content, and secrets other than PEM keys. Output reads "no findings in supported categories", never "clean".
- **Safe failure policy:** `dir`/`staged` — report incomplete, exit 3 for in-scope failures; `push` — fail closed on internal errors, missing objects, decode/OCR failures, unsupported formats warn by default; `redact` — never overwrite, write nothing for a failed file, report residual findings after rescan.

## 7. Decisions and evidence

**Confirmed (accessed 2026-10-26):**
- gitleaks: MIT, "feature complete", security patches only; author moving to Betterleaks — https://github.com/gitleaks/gitleaks
- Betterleaks: MIT, Go, secrets only, pre-commit hook, v2 unreleased — https://github.com/betterleaks/betterleaks
- TruffleHog: AGPL-3.0, secrets + verification, no OCR/PII — https://github.com/trufflesecurity/trufflehog
- piilint: Apache-2.0, text PII, `--staged`, redacted copies, no OCR, 0.1.0 on PyPI — https://libraries.io/pypi/piilint
- Presidio image redactor: alpha, needs Tesseract + spaCy — https://pypi.org/project/presidio-image-redactor/0.0.53
- screenpipe PII image model: CC BY-NC 4.0, so not bundleable in permissive OSS — https://huggingface.co/screenpipe/pii-image-redactor
- ocrs: MIT/Apache-2.0, Latin-only, early preview, downloads models to `~/.cache/ocrs` — https://github.com/robertknight/ocrs
- gitoxide read crates "usable", not production — https://github.com/GitoxideLabs/gitoxide
- pre-push stdin format, all-zero OIDs for new/deleted refs — https://git-scm.com/docs/githooks
- Name collisions — https://opencollective.com/leaklens/updates/leaklens-is-live-open-registry-breach-checker-and-cve-tracking ; VS Code extension seen only in a LinkedIn post (unverified repo)

**Assumptions:** `git` on PATH; tens-of-MB binary acceptable; Latin-only OCR acceptable for v0.1; 4-core laptop reference CPU.

**Unresolved:** ocrs model-weight and training-data license; rten loading from bytes; minimum `git` version for `--missing=print`; `phonenumber` crate license/size; pre-commit `language: rust` behavior; which clipboard/agent log formats to prioritize; final name (check crates.io, GitHub, Homebrew, domain, trademark).

**Next implementation task:** T1 and T2 in parallel — the S1 verdict gates T4.

