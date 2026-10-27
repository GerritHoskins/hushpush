# S1 validation

Measured 2026-10-27 from this spike directory. All repository writes were confined to `spikes/s1-ocr/`; the explicitly requested Cargo installation and model acquisition use their standard user caches. Root workspace files were not edited. Nothing was committed, pushed or published.

| Command / check | Result |
| --- | --- |
| `cargo install ocrs-cli --locked` | PASS; installed 0.13.1. Its upstream locked graph warned about yanked clipboard-win/spin versions; no unlocked retry. |
| `npm ci` | PASS; two packages installed, audit reported zero vulnerabilities. |
| `npm run gen` | PASS; generated all 60 screenshots and truth files using installed Edge, without browser downloads. |
| `node verify-repro.mjs` | PASS; all 60 PNG hashes, canonical truth hashes and raw truth JSON byte hashes matched before/after regeneration. |
| `npm run run` | PASS; all 60 baseline and 60 fallback CLI invocations completed; the separate warm process measured every image in both variants. |
| `npm run score` | PASS; regenerated RESULTS.md and masked scores.json from the complete measured artifacts. |
| `npm test` | PASS; 10 tests covering masking, Luhn, look-alikes, synthetic formats, split word separators, line boundaries, endpoint selection, geometry scaling and rectangle union. |
| `cargo fmt --check` in `bench/` | PASS. |
| `cargo clippy --locked --all-targets -- -D warnings` in `bench/` | PASS. |
| `cargo test --locked` in `bench/` | PASS; benchmark binary has zero Rust unit tests. Behavioral scorer tests are the Node suite above. |
| `cargo deny check` in `bench/` | PASS: advisories, bans, licenses, sources. Warnings: duplicate bitflags/miniz_oxide versions and unused license allowances. This audits code dependencies, not downloaded model-weight terms. |
| `cargo build --release --locked` in `bench/` | PASS. |
| `node --check` on generator, runner, scorer | PASS. |
| `git check-ignore` on corpus, node_modules, bench target | PASS; generated data/dependencies/build products are ignored by the spike-local .gitignore. |

Node dependencies were checked from installed package metadata: playwright-core 1.56.1 is Apache-2.0; pngjs 7.0.0 is MIT. Rust dependencies pass the spike-local permissive license allowlist.

During scorer verification, a test exposed accidental numeric-record merging when line breaks were flattened. The final scorer normalizes within lines and preserves line boundaries; the regression test passes. Cards split across OCR lines remain a documented detector limitation. Early scoring attempts before the asynchronous warm run finished correctly failed due to missing timing artifacts; final scoring passed after both variants completed.

The harness/report acceptance criteria are met. The OCR hypothesis is **FAIL**: baseline and first fallback do not meet recall/localization targets. Desktop timing results are in RESULTS.md, with a separate explicit NOT RUN for the four-core laptop. No laptop, network-disabled packaging or release-archive size test was performed. Model-weight redistribution remains unresolved because the upstream model card uses CC-BY-SA-4.0 and exact S3 artifact provenance is unconfirmed.
