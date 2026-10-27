> Renamed to hushpush on 2026-10-27; historical prose and results below are unchanged.

# S1 screenshot OCR spike

From this directory in PowerShell 7, with Node/npm, Rust stable and Microsoft Edge already installed:

```powershell
npm ci && npm run gen
cargo install ocrs-cli --version 0.13.1 --locked
npm run run && npm run score
```

Ensure `%USERPROFILE%\.cargo\bin` is on PATH for the install command. The harness itself resolves Cargo and ocrs from that directory. No browser download: Playwright launches installed Edge with `channel: "msedge"`. The first OCR invocation downloads the default models to `~/.cache/ocrs`; later invocations reuse them. All screenshot/truth/OCR/measurement artifacts stay under ignored `out/`. Models are not bundled. `RESULTS.md` is regenerated from measured artifacts by `npm run score`.

The bench is its own Cargo workspace, independent of the root. npm and Cargo lockfiles are deliberately local and pin this new spike's dependencies; root manifests and lockfiles are untouched.

Additional verification: `npm test`, `node verify-repro.mjs`, and from `bench/`: `cargo fmt --check`, `cargo clippy --locked --all-targets -- -D warnings`, `cargo test --locked`. Reproducibility applies to repeated generation on the recorded Edge/OS/fonts; installed Edge or font updates may change PNG bytes. Timings are intentionally not reproducible bytes.

Do not point this harness at real files. It only consumes its generated manifest. Synthetic values exist in the explicitly requested truth and raw OCR artifacts; scoring summaries and the report contain only IDs, categories and metrics. The scorer does not apply the product's example-domain suppression because reserved domains are the positive email fixtures here.
