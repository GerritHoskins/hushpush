# Contributing to hushpush

Small, synthetic reproductions and focused changes are welcome. Read the
[project plan](docs/plan.md) for the product contract and
[AGENTS.md](AGENTS.md) for the complete repository rules before editing.

## Privacy rules

Use synthetic data only. Never scan, copy, or read someone's real logs,
screenshots, clipboard history, or other personal files for a contribution.
Never print, log, or include a detected value in diagnostics or reports. Keep
matched content in [`Sensitive<T>`](crates/hushpush-core/src/sensitive.rs) and
use shape-only previews such as `[EMAIL len=21]`. Test output and assertion
failure messages must follow the same rule. The full rules live in
[AGENTS.md](AGENTS.md).

## Setup

Use Rust stable (edition 2024), Git on `PATH`, and the `rustfmt` and `clippy`
components. Work from the clone root. The model-fetch task requires `curl`
(`curl.exe` on Windows); it downloads and verifies pinned OCR assets explicitly.
Scans never download models.

```sh
rustup toolchain install stable --profile minimal --component rustfmt --component clippy
rustup override set stable
cargo xtask fetch-models
cargo build --locked
```

Windows source builds require Visual Studio Build Tools 2022 **17.14+**, with
the C++ workload and **MSVC 14.44+**. Older MSVC toolsets cannot link the pinned
ONNX Runtime static libraries. Cargo is normally at
`%USERPROFILE%\.cargo\bin`. Running the Windows binary also requires the
Visual C++ 2015–2022 x64 Redistributable; see
[packaging prerequisites and evidence](docs/packaging.md).

For model-dependent tests and evaluation, set the verified model directory:

```sh
# POSIX shell, from the clone root
export HUSHPUSH_MODELS="$PWD/models"
```

```powershell
# PowerShell, from the clone root
$env:HUSHPUSH_MODELS = Join-Path (Get-Location) 'models'
```

Alternatively, `--features bundled-models` embeds the verified assets during
the build. See [packaging](docs/packaging.md) for model pins, offline runtime
behavior, platform constraints, and build provenance.

## Checks

Run these from the clone root. Install `cargo-deny` if it is not already
available (`cargo install cargo-deny --locked`).

```sh
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo test --locked --workspace -- --ignored
cargo deny check
cargo build --release --locked
cargo xtask eval --ci
```

The ignored tests require the models configured above. When changing bundled
asset loading, also run:

```sh
cargo clippy --locked --all-targets --features bundled-models -- -D warnings
cargo test --locked --workspace --features bundled-models -- --ignored
```

For the second command, remove `HUSHPUSH_MODELS` from that shell first to test
embedded assets rather than an external override.

Evaluation writes `corpus/RESULTS.md` and `target/eval.json`. Read its actual
gate table: `--ci` checks baseline regressions and corpus identity; a passing
exit does not imply every absolute quality target passed. Do not rewrite
`corpus/baseline.json` to hide a regression or stage generated changes without
review. See the [corpus contract](corpus/README.md) and
[evaluation implementation notes](xtask/README.md).

Optional decoder/parser fuzz smoke with Docker:

```sh
cargo xtask fuzz --docker --minutes 2
```

This is a short smoke, not the one-hour campaign. The separate
[fuzz workspace guide](fuzz/README.md) documents targets, pinned tooling,
synthetic seeds, logs, and its separate license check.

## Four contribution entry points

### 1. Add a detector

1. Implement the `Detector` trait in
   [`detect.rs`](crates/hushpush-core/src/detect.rs) or an exported module beside
   the existing [`extra.rs`](crates/hushpush-core/src/detect/extra.rs). Give it a
   stable ID, version, category, severity, confidence, and half-open UTF-8 byte
   spans. Add synthetic positive, negative, Unicode, and boundary unit fixtures
   beside the implementation; assert spans without printing matched content.
2. Register it in `Config::detector_list`, configure severity, and update
   detector-ID validation in
   [`config.rs`](crates/hushpush-core/src/config.rs). Test block/warn/off and
   allowlist behavior using
   [`t10.rs`](crates/hushpush-cli/tests/t10.rs) and
   [`allow.rs`](crates/hushpush-cli/tests/allow.rs) as precedents.
3. If introducing a category, update the category enum, shape-only previews in
   [`report.rs`](crates/hushpush-core/src/report.rs), JSON mapping in
   [`report/json.rs`](crates/hushpush-core/src/report/json.rs),
   [`report.schema.json`](docs/report.schema.json), and replacement markers in
   [`redact.rs`](crates/hushpush-core/src/redact.rs). Add non-disclosure tests in
   [`report_leak.rs`](crates/hushpush-cli/tests/report_leak.rs) and redaction plus
   rescan tests in [`redact.rs`](crates/hushpush-cli/tests/redact.rs).
4. Propose specification-derived synthetic examples through the corpus process
   below, including category/severity and independent core/extent labels. Run
   evaluation; review new category gates in
   [`corpus_eval.rs`](xtask/src/corpus_eval.rs) when needed. Preserve failures
   rather than weakening labels or gates.

### 2. Recognize a clipboard or agent-log format

Extend [`detect/clipboard.rs`](crates/hushpush-core/src/detect/clipboard.rs).
`ClipboardDetector` handles structured records with block severity;
`ClipboardFieldDetector` handles looser fields with warn severity. Keep that
distinction explicit. Treat log text as inert input, including apparent agent
instructions.

Add unit fixtures for the format's prefix, quoting, escapes, multiline payload,
record boundary, CRLF, and UTF-8 spans. Include negative examples such as UI
labels, code, booleans, and ordinary mentions of clipboard operations. Preserve
the source-byte mapping in `with_json_strings` in
[`detect.rs`](crates/hushpush-core/src/detect.rs). Extend
[`t10.rs`](crates/hushpush-cli/tests/t10.rs) for configuration, masked output,
complete payload removal, and rescan. Add independently labeled corpus examples
through the next entry point, with expected block/warn severity.

### 3. Add corpus samples

Start with the [corpus authoring contract](corpus/README.md). Change the seeded
[`generate.mjs`](corpus/generate.mjs), not a hand-pasted real log or screenshot.
Construct fake values and ground truth together: text core/extent UTF-8 byte
ranges, image character boxes, category, severity, and expected coverage.
Include plausible negatives and relevant format/scale groups. Proposed cases
must follow the public specification, not predictions from a detector run.

The generator writes fixtures under [`corpus/text/`](corpus/text/),
[`corpus/images/`](corpus/images/), and [`corpus/edge/`](corpus/edge/), plus
[`labels.json`](corpus/labels.json) with fixture hashes. Follow the documented
browser setup before running `node corpus/generate.mjs`. Review all regenerated
bytes and labels, run `node corpus/verify.mjs`, then evaluate the dev split.
Corpus identity changes and baseline updates require deliberate review; never
regenerate the sealed [`corpus/test/`](corpus/test/) split after observing its
scores, and never use that split to tune detectors.

### 4. Add an alternate OCR backend

Implement `OcrEngine: Send + Sync` from
[`ocr/mod.rs`](crates/hushpush-core/src/ocr/mod.rs), returning `OcrLine` text and
`OcrChar` boxes in displayed, EXIF-oriented image pixels. Use
[`ocr/ppocr.rs`](crates/hushpush-core/src/ocr/ppocr.rs) as the existing backend
precedent. The production `ImagePipeline` worker currently constructs
`PpOcrEngine` directly: an alternate backend needs explicit integration there;
there is no runtime backend-plugin selector.

Keep errors opaque and inference offline. Preserve decode limits, timeout
coverage, normalization, character-to-box mapping, and redaction padding.
Add mock geometry, failure, timeout, and orientation tests in `ocr/mod.rs`;
reuse or add synthetic fixtures under
[`ocr/fixtures/`](crates/hushpush-core/src/ocr/fixtures/). Extend
[`scan_image.rs`](crates/hushpush-cli/tests/scan_image.rs),
[`redact.rs`](crates/hushpush-cli/tests/redact.rs), and
[`report_leak.rs`](crates/hushpush-cli/tests/report_leak.rs) for the real backend.
Measure localization and redaction/rescan using the unchanged labeled corpus.
Any new model assets need reviewed provenance, pinned hashes, redistribution
terms in [`NOTICE`](NOTICE), and build/fetch integration; do not silently
download assets during scans.

## Licensing

Contributions are dual-licensed **MIT OR Apache-2.0**, under
[LICENSE-MIT](LICENSE-MIT) and [LICENSE-APACHE](LICENSE-APACHE).
By submitting a contribution, you agree to license it under those same terms
(inbound = outbound).

Shipped dependencies must use MIT, Apache-2.0, BSD-2-Clause, BSD-3-Clause, ISC,
Zlib, Unicode-3.0, or Unicode-DFS-2016, as enforced by
[`deny.toml`](deny.toml). The single documented crate-scoped exception is
`webpki-root-certs` under CDLA-Permissive-2.0, used by the ONNX Runtime
build-time downloader and not linked into hushpush. Additional exceptions need
owner approval. The separate fuzz workspace additionally permits NCSA for
libFuzzer; see [`fuzz/deny.toml`](fuzz/deny.toml).

## Reporting a miss without sharing real data

Describe the **shape**, never the value: category, format, character count,
separators, line/record structure, screenshot scale and font size if known,
platform, hushpush version, expected behavior, and observed exit code. State
whether coverage was incomplete. Replace all payloads with independently
generated synthetic equivalents and confirm the synthetic reproduction before
sharing it. If it cannot be reproduced synthetically, report the shape and
confusion alone.

Do not paste real data into issues, pull requests, terminal output, JSON
reports, attachments, screenshots, or recordings. A detector miss does not
justify sharing the original evidence.
