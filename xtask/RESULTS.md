# T4 product OCR evaluation

Measured 2026-10-27 on the synthetic corpus only. No spike files were modified.
Raw aggregate output: `eval-results.txt`.

Command: `cargo run --release -p xtask -- eval-ocr` (exit 0, PASS).
All 60 image byte hashes and 60 compact-JSON truth hashes match the frozen
manifest. Both variants use product decode, PP-OCR inference, per-character
NFKC, shared email/card detectors with reserved domains included, and +8 px
char-span union boxes in original image pixels. Phone is not scored because
its detector does not exist. No scorer-assisted OCR correction is performed.

| Variant | Scale | Email | Card | Warm mean / max seconds |
| --- | --- | --- | --- | --- |
| Baseline | 1 | 49/60 (81.67%) | 56/60 (93.33%) | 0.877 / 1.226 |
| Baseline | 1.5 | 60/60 (100%) | 58/60 (96.67%) | 0.883 / 1.169 |
| Baseline | 2 | 60/60 (100%) | 60/60 (100%) | 0.886 / 1.136 |
| Upscale small images 2x | 1 | 56/60 (93.33%) | 55/60 (91.67%) | 1.181 / 1.546 |
| Upscale small images 2x | 1.5 | 60/60 (100%) | 58/60 (96.67%) | 0.876 / 1.141 |
| Upscale small images 2x | 2 | 60/60 (100%) | 60/60 (100%) | 0.880 / 1.129 |

Decision: KEEP small-image upscale. Combined 1x found count rises from 105 to
111; email improves, card loses one occurrence, and each category remains
above the 85% 1x gate. Both categories meet the 95% 2x gate. Every found value
has 100% coverage of the planted browser glyph rectangle union at +8 px.
All variants/scales have zero false findings on benign look-alikes. Unmatched
email findings at 1x: baseline 5, upscale 4; all other false counts are zero.
The upscale configuration passes all requested measured gates; baseline fails
1x email recall.

CPU: AMD Ryzen 7 7800X3D, Windows, 16 logical CPUs reported by the host.
ORT CPU only, four intra-op threads, one inter-op thread, sequential execution.
First-image warmup excluded; decode, resize, inference and detection included.
One warm sample per image; 20 images per scale. Speed gate applies to 1x
1080p-class originals. These are desktop measurements, not laptop or CI-OS
performance evidence. Network isolation was not tested.

OCR hardening rejects excessive contour counts and bounds crop/recognizer
allocations; caught OCR panics become incomplete coverage. One timed-out
worker is retired without spawning replacements.

Other checks on Windows: cargo fmt --all -- --check PASS;
cargo clippy --workspace --all-targets -- -D warnings PASS;
cargo test --workspace PASS (78 passed, 2 model tests ignored);
LEAKLENS_MODELS=<repo>/models cargo test --workspace -- --ignored PASS
(2 passed); cargo deny check PASS (warnings only, no advisory exceptions).
Fetch-models PASS with SHA-256 verified weights/dictionary/official runtime.
Synthetic demo reports masked Medium-confidence email/card bbox findings;
model integration also covers staged, commits and push scan sources.

Supporting Cargo alias, model ignore rule and three-OS CI steps are prepared
in `supporting.patch`, not applied because the task restricts writes to crates,
root Cargo files and xtask. The scope question is pending. macOS/Linux model
execution and CI remain unverified locally. No repository commit or push.
