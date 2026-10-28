# Building from source

Rust stable, edition 2024; run from the clone root.


```bash
cargo xtask fetch-models          # SHA-256 verified models only
cargo build --release --locked -p hushpush-cli --features bundled-models
./target/release/hushpush scan dir ./demo/evidence --include-reserved --no-user-patterns
```

`cargo install hushpush-cli` and default-feature builds do not embed models. Run `cargo xtask fetch-models` from this checkout, then pass `--models DIR` or set `HUSHPUSH_MODELS`. Model lookup is `--models` > `HUSHPUSH_MODELS` > embedded models > models beside the canonicalized executable > debug checkout models. Explicit paths override embedded assets even if invalid. Build-time `HUSHPUSH_MODELS_DIR` changes the source directory for embedding; missing or modified files fail the build. Source builds link a prebuilt static ONNX Runtime and need a current C++ toolchain: on Windows, Visual Studio 2022 17.14 or newer (MSVC 14.44+); older MSVC fails to link with unresolved `__std_*` symbols.


See [CONTRIBUTING](../CONTRIBUTING.md) for checks and contribution entry points.

CI and release builds run `bash scripts/fetch-ort.sh [target [archive-directory]]` before compiling, preserving ort-sys's normal linking and reusing existing runtime directories untouched. Archives (`ORT_ARCHIVE_DIR`, default runner temp or home cache) are verified against the locked ort-sys SHA-256 before extraction. Downloads allow four 600-second attempts, 30-second connect/stall limits (32 KiB/s threshold), and 5/10/20-second backoff. CI caps the step at 45 minutes; cargo-dist 0.33.0 omits this field when generating release steps.

## Binary installers after the first release

These are configured distribution entry points, **not currently available
installations**. No release or tag exists yet. Once an approved release exists,
review the installer script before executing it:

```sh
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/GerritHoskins/hushpush/releases/latest/download/hushpush-cli-installer.sh | sh
# Alternatives, after publication:
brew install GerritHoskins/tap/hushpush
cargo binstall hushpush-cli
```

```powershell
irm https://github.com/GerritHoskins/hushpush/releases/latest/download/hushpush-cli-installer.ps1 | iex
```
