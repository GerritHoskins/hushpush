# OCR development tasks

## Independent corpus evaluation

Build the product with `cargo build --release`, set `HUSHPUSH_MODELS` to the
downloaded models directory, then run `cargo xtask eval`. It writes the measured
gate table and grouped precision/recall to `corpus/RESULTS.md` and full machine
results to `target/eval.json` (or `$CARGO_TARGET_DIR/eval.json`). Absolute gate
failures are results and do not change the command's successful exit status.
Operational scan/configuration errors still fail.

Only files listed in `corpus/labels.json` are fixtures. The harness stages their
unchanged bytes (preserving symlinks) in a unique temporary directory, invokes
the release CLI with `scan dir --json - --include-reserved --no-allow`, and loads
the canonical corpus patterns with `HUSHPUSH_PATTERNS`. The non-Git temporary
working directory respects the product's rejection of repository-owned user
patterns and avoids scanning generator source, truth or previous results as
test data. The staging directory uses `tempfile` and is removed on drop, including error paths.

`cargo xtask eval --split dev --write-baseline` deliberately records the current quality
metrics and fixture/pattern identity in `corpus/baseline.json`. Review both
generated files. `cargo xtask eval --split dev --ci` fails if a recorded quality metric
falls more than one percentage point, is missing, or the fixture identity
differs. It never rewrites the baseline. Speed gates remain visible but are not
regression gates because CPU speed differs between runners.

`--split dev` is the default. `--split test` prints a milestone-only reminder,
checks its manifest against the dev baseline, and writes `corpus/test/RESULTS.md`
and `target/eval-test.json`. Test cannot use `--ci` or `--write-baseline`.
Off-spec phone/key groups are reported separately and excluded from quality
gates and regression metrics. Spill outside text extents is diagnostic only.

Matches require exact schema category and severity, with at least 90% of core bytes covered or 98% planted glyph-union
coverage under the product's already padded image box. A prediction can match
only one plant. UTF-8 byte labels are compared by translating the public
report's Unicode columns. Reports include false findings on negative regions,
edge coverage classes, omitted redaction outputs and phone OCR at 1x/2x.
The redaction check compares both original and staged input hashes and rescans
every retained output; a separate gate exposes missing supported copies.

The uncommitted 50,000,000-byte throughput log uses only a text-size limit
override. Warm image timing runs the unchanged product image pipeline after one
excluded warmup, including decode/OCR/detection and excluding model/process
startup. Quality scoring always uses the release CLI JSON report.

Run `cargo run --release -p xtask -- fetch-models` from the checkout. It requires `curl` and `tar`
(on Windows it uses the bundled `curl.exe` and `%WINDIR%/System32/tar.exe`).
The scanner itself never invokes either tool or accesses the network.

The task downloads the official PP-OCRv5 mobile ONNX detector/recognizer and
model cards at the exact Hugging Face revisions recorded in
`spikes/s1b-ppocr/provenance.json`. It derives the dictionary from the verified
recognizer configuration, preserving entry order and CRLF line endings to
match the provenance SHA-256 on every OS. Each installed asset is checked
against its pinned SHA-256, including on cache hits. Downloads use HTTPS with
certificate verification. Windows tolerates unavailable certificate revocation
servers through curl's `--ssl-revoke-best-effort` option.

The official Microsoft ONNX Runtime **CPU** 1.22.0 release archive is downloaded
for the host OS/architecture. Supported hosts are Windows x64, Linux x64/arm64,
and macOS x64/arm64. SHA-256 pins in `src/fetch.rs` were computed from the
corresponding official GitHub release archives. The complete extracted archive
(including MIT license and third-party notices) remains under `models/ort/`.
Model cards declaring Apache-2.0 remain under `models/`. Nothing is fetched
from mutable `main` or `latest` URLs.

Downloads and the runtime cache live in `models/`. The supporting patch below adds its ignore rule.
The runtime libraries are also copied beside debug/release executables and
test executables under `deps/`, respecting `CARGO_TARGET_DIR` when set.
The command prints the absolute library path for `ORT_DYLIB_PATH` as an
alternative. To use another binary directory, copy the printed runtime library
and its companion libraries there, or set `ORT_DYLIB_PATH`. The scanner's
model lookup accepts `--models`, `HUSHPUSH_MODELS`, and packaged model paths.

`cargo run --release -p xtask -- eval-ocr` evaluates the product pipeline against the unchanged
synthetic S1 corpus. Use release optimization for speed measurements; the Cargo
alias in the supporting patch selects the release profile. See the command output for its arguments,
coverage metrics, recall gates, CPU/thread information, and limitations.

`supporting.patch` is a checked, unapplied patch for the Cargo alias, models
ignore rule and cached three-OS model integration-test CI steps. These paths
are outside T4's assigned ownership; application awaits the scope decision.
`git apply --check xtask/supporting.patch` passes. Once applied, use
`cargo xtask fetch-models` and `cargo xtask eval-ocr`.
