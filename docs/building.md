# Building from source

Rust stable, edition 2024; run from the clone root.


```bash
cargo xtask fetch-models          # SHA-256 verified models only
cargo build --release --locked -p hushpush-cli --features bundled-models
./target/release/hushpush scan dir ./demo/evidence --include-reserved --no-user-patterns
```

Default-feature builds, including `cargo install --locked --path crates/hushpush-cli` from this checkout, do not embed models. Run `cargo xtask fetch-models` from this checkout, then pass `--models DIR` or set `HUSHPUSH_MODELS`. Model lookup is `--models` > `HUSHPUSH_MODELS` > embedded models > models beside the canonicalized executable > debug checkout models. Explicit paths override embedded assets even if invalid. Build-time `HUSHPUSH_MODELS_DIR` changes the source directory for embedding; missing or modified files fail the build. Source builds link a prebuilt static ONNX Runtime and need a current C++ toolchain: on Windows, Visual Studio 2022 17.14 or newer (MSVC 14.44+); older MSVC fails to link with unresolved `__std_*` symbols.


See [CONTRIBUTING](../CONTRIBUTING.md) for checks and contribution entry points.

CI and release builds run `bash scripts/fetch-ort.sh [target [archive-directory]]` before compiling, preserving ort-sys's normal linking and reusing existing runtime directories untouched. Archives (`ORT_ARCHIVE_DIR`, default runner temp or home cache) are verified against the locked ort-sys SHA-256 before extraction. Downloads allow four 600-second attempts, 30-second connect/stall limits (32 KiB/s threshold), and 5/10/20-second backoff. CI caps the step at 45 minutes; cargo-dist 0.33.0 omits this field when generating release steps.

## Binary installers

Releases since [`v0.1.0`](https://github.com/GerritHoskins/hushpush/releases/tag/v0.1.0)
are published through these routes. hushpush is not published on crates.io, so
there is no `cargo install hushpush-cli` or `cargo binstall` route; build from
source as above instead. Review the installer script before executing it; the
shell and PowerShell installers place `hushpush` in `$CARGO_HOME/bin` (default
`~/.cargo/bin`):

```sh
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/GerritHoskins/hushpush/releases/latest/download/hushpush-cli-installer.sh | sh
# Alternative on macOS and Linux:
brew install GerritHoskins/tap/hushpush
```

```powershell
irm https://github.com/GerritHoskins/hushpush/releases/latest/download/hushpush-cli-installer.ps1 | iex
```

With Node.js and npm installed, from npmjs.com:

```sh
npm install -g @gerrithoskins/hushpush
hushpush --version
npx @gerrithoskins/hushpush --version
```

The npm package downloads the platform archive from GitHub Releases at install
time; scanning remains offline. Neither npm registry receives pre-releases.

### GitHub Packages

To use GitHub Packages instead of npmjs.com, put this in your user `.npmrc`
(use an environment variable, never commit a token):

```ini
@gerrithoskins:registry=https://npm.pkg.github.com
//npm.pkg.github.com/:_authToken=${GITHUB_PACKAGES_TOKEN}
```

Set `GITHUB_PACKAGES_TOKEN` in your shell to a GitHub personal access token
(classic) with `read:packages` and access to the package, then use the same
`npm install -g` or `npx` commands above. Authentication is required even for
public GitHub Packages. This scope mapping routes all `@gerrithoskins` packages
to GitHub Packages; remove it to return to npmjs.com. See
[GitHub's npm registry documentation](https://docs.github.com/en/packages/working-with-a-github-packages-registry/working-with-the-npm-registry).
