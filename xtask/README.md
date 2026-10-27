# OCR development tasks

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
model lookup accepts `--models`, `LEAKLENS_MODELS`, and packaged model paths.

`cargo run --release -p xtask -- eval-ocr` evaluates the product pipeline against the unchanged
synthetic S1 corpus. Use release optimization for speed measurements; the Cargo
alias in the supporting patch selects the release profile. See the command output for its arguments,
coverage metrics, recall gates, CPU/thread information, and limitations.

`supporting.patch` is a checked, unapplied patch for the Cargo alias, models
ignore rule and cached three-OS model integration-test CI steps. These paths
are outside T4's assigned ownership; application awaits the scope decision.
`git apply --check xtask/supporting.patch` passes. Once applied, use
`cargo xtask fetch-models` and `cargo xtask eval-ocr`.
