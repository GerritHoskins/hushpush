# T14 packaging evidence

Status: implemented and measured on Windows and Linux; macOS is not yet measured. T14 acceptance (fresh-VM install to first result <=60 s on 3 OSes) is **not verified**: the Windows timing ran on a host that already had the VC++ redistributable, Linux ran in containers, and macOS has no run. No tag or release has been made. The owner created the tap repository and the `HOMEBREW_TAP_TOKEN` secret. Base checkout: `e2c3c56d5f12b1ea730db282d718687f0fd80334`. Measurements below describe synthetic fixtures only.

## Build contract

`hushpush-cli/bundled-models` forwards to core and is off by default. Core `build.rs` reads `HUSHPUSH_MODELS_DIR` (otherwise workspace `models/`), verifies the detector, recognizer and dictionary against `crates/hushpush-core/model_pins.rs`, and copies verified bytes to OUT_DIR for `include_bytes!`. `xtask fetch-models` uses the same pins and fetches no runtime. Both ONNX sessions load from memory. Runtime model lookup: explicit `--models` > `HUSHPUSH_MODELS` > embedded > `models` beside canonicalized current executable > debug checkout path. Explicit invalid overrides fail closed, without falling back. Version output identifies embedded assets and short hashes.

`ort =2.0.0-rc.10`, `std,download-binaries`, default features disabled, links ONNX Runtime 1.22.0 CPU libraries. There is no dylib loader, runtime side file or ORT_DYLIB_PATH configuration. Session construction retains a catch_unwind with an opaque error. Scanning never downloads anything.

### Prebuilt static libraries

Source: ort-sys rc.10's shipped `dist.txt` and `build.rs`. URL pattern:
`https://cdn.pyke.io/0/pyke:ort-rs/ms@1.22.0/<rust-target>.tgz`.
All five requested CPU (`none`) target entries exist. Windows and Linux x64 archives were fetched by actual builds; the other three are verified as listed entries, not locally built acceptance results. ort-sys downloads using HTTPS, checks SHA-256 against the table before extraction, and links static archives.

| Target | Archive SHA-256 |
|---|---|
| x86_64-unknown-linux-gnu | ed1716de95974bf47ab0223ca33734a0b5a5d09a181225d0e8ed62d070aea893 |
| aarch64-unknown-linux-gnu | 24e4760207136fc50b854bb5012ab81de6189039cf6d4fd3f5b8d3db7e929f1e |
| x86_64-apple-darwin | e0538783248bbb77d2e97556134b8cc47cfc35ac3d762067d51eaf8355c0f48e |
| aarch64-apple-darwin | 00fbfd6f08bac2a4e28c66723af900d58d1b4b1c73efba6290637cd3019883d5 |
| x86_64-pc-windows-msvc | 540d19b3379fda6fb8f7280d8c15efde20ed225a67a357a6dae38c4300fe190d |

### License evidence

`NOTICE` includes PaddlePaddle Apache-2.0 model source revisions, full model/dictionary hashes and ONNX Runtime's MIT attribution. Archive `ThirdPartyNotices` is an unmodified copy of
[ONNX Runtime v1.22.0 ThirdPartyNotices.txt](https://raw.githubusercontent.com/microsoft/onnxruntime/v1.22.0/ThirdPartyNotices.txt), SHA-256 `e9e90971a8e75a9a8ac0c6412e29c1202d079998389915aa485f46c816c3b4cc`.
The collection includes optional components; its contents do not establish which optional components pyke compiled.

The build-time chain `ort-sys -> ureq -> webpki-root-certs` includes `CDLA-Permissive-2.0`. The owner approved one crate-scoped cargo-deny exception for `webpki-root-certs` only (root and fuzz `deny.toml`, recorded in `AGENTS.md`); the allowlist is otherwise unchanged. The crate is used only by ort-sys's build-time downloader and is not linked into hushpush.

## Release automation

Pinned cargo-dist: **0.33.0**, published 2026-09-11 (46 days before this task), from the [cargo-dist release history](https://axodotdev.github.io/cargo-dist/changelog/). Config lives in `dist-workspace.toml`; `release.yml` is generated. Supported build-setup hook `.github/build-setup.yml` runs `cargo xtask fetch-models` before compilation. The dist profile enables bundled models; all five targets remain configured. Archives include LICENSE-MIT, LICENSE-APACHE, README.md, NOTICE and ThirdPartyNotices. Default archive layout also matches cargo-binstall lookup.

Owner provisioning: create **GerritHoskins/homebrew-tap** and add the repository secret **HOMEBREW_TAP_TOKEN** with permission to publish there. Formula name: `hushpush`. No provisioning or publication occurred here.

`dist plan` and `dist generate --check` passed. Planned archives:

- hushpush-cli-x86_64-unknown-linux-gnu.tar.xz
- hushpush-cli-aarch64-unknown-linux-gnu.tar.xz
- hushpush-cli-x86_64-apple-darwin.tar.xz
- hushpush-cli-aarch64-apple-darwin.tar.xz
- hushpush-cli-x86_64-pc-windows-msvc.zip

`dist build --artifacts global` passed. Generated installer artifacts: `hushpush-cli-installer.sh` (59,817 bytes), `hushpush-cli-installer.ps1` (22,389 bytes), and `hushpush.rb` (2,161 bytes). The generated source archive uses committed HEAD and is not evidence for this uncommitted implementation.

### Windows: measured on 2026-10-27 (coordinator)

Toolchain: Visual Studio Build Tools 2022 17.14, MSVC **14.44.35207**. MSVC 14.42 cannot link pyke's static library (unresolved `__std_*` STL helpers such as `__std_find_last_of_trivial_pos_1`); 17.14+ is the minimum for Windows source builds.

`dist build --artifacts local --target x86_64-pc-windows-msvc` produced `hushpush-cli-x86_64-pc-windows-msvc.zip`: **27,087,229 bytes** (below 60 MB); `hushpush.exe` is **45,391,360 bytes**. `msvc-crt-static = false` stays, because pyke's library is built against the dynamic CRT.

`python docs/windows-install-smoke.py target/distrib/hushpush-cli-x86_64-pc-windows-msvc.zip`, three runs (fresh temp directory, PATH limited to System32, no HUSHPUSH/CARGO/ORT variables):

| Run | Extract (s) | Scan (s) | Total (s) | Exit | Screenshot email bbox |
|---|---:|---:|---:|---:|---|
| 1 | 0.166 | 0.492 | 0.659 | 1 | yes |
| 2 | 0.167 | 0.533 | 0.700 | 1 | yes |
| 3 | 0.186 | 0.497 | 0.683 | 1 | yes |

Load-time DLL imports (`dumpbin /dependents`), beyond kernel32/ntdll/advapi32/setupapi/dbghelp/bcryptprimitives and the Universal CRT:
- `MSVCP140.dll`, `MSVCP140_1.dll`, `VCRUNTIME140.dll`, `VCRUNTIME140_1.dll`: the Visual C++ 2015–2022 redistributable. **Not part of a clean Windows install.** Users without it get a Windows loader error before hushpush starts, so hushpush cannot print its own hint. The README states the prerequisite.
- `DirectML.dll`, `d3d12.dll`, `dxgi.dll`, `dxcore.dll`: linked eagerly by pyke's Windows build although hushpush uses only the CPU execution provider. They ship with Windows 10 1903+ and Windows 11 desktop editions, so they may be missing on Windows Server Core or Nano containers.

**Not measured:** a Windows system without the VC++ redistributable (Windows Sandbox is not enabled on the measuring host), and the PowerShell installer path (`install-smoke.yml` covers it after a release). The measuring host already had the redistributable, so this timing does not prove a fresh-system install.

### Linux: measured on 2026-10-27

Official Rust build image: `rust:1.90-bookworm@sha256:3914072ca0c3b8aad871db9169a651ccfce30cf58303e5d6f2db16d1d8a7e58f` (Docker Hub official `rust`, pinned manifest digest). Command: `cargo build --release --locked -p hushpush-cli --features bundled-models --target x86_64-unknown-linux-gnu`. Reproduction: `docs/linux-install-smoke.ps1`; synthetic fixture generator: `docs/generate-install-demo.py`.

The stripped bundled **release** binary is **48,249,864 bytes**; the locally assembled `hushpush-x86_64-unknown-linux-gnu.tar.gz` archive is **28,775,920 bytes** (below 60 MB). This is a measured manual Linux smoke archive, not a generated cargo-dist `.tar.xz` artifact; no size is claimed for unbuilt dist archives.

Both runtime images were unchanged, had nothing installed, and ran with `--network none`, `env -i`, no HUSHPUSH/CARGO/ORT overrides, and no model/runtime side files. A synthetic fixture directory was mounted read-only. Timing starts immediately before extraction and ends when the first scan exits, excluding download/build/container startup. Reserved demo email domains require `--include-reserved`.

| Clean runtime | glibc | Extract (s) | Scan (s) | Total (s) | Exit | Screenshot email bbox |
|---|---|---:|---:|---:|---:|---|
| Debian bookworm-slim | 2.36 | 0.551470780 | 1.092787675 | 1.644258455 | 1 | [21,96,313,56] |
| Ubuntu 22.04 | 2.35 | 0.537516258 | 1.014133374 | 1.551649632 | 1 | [21,96,313,56] |

Runtime digests:
- `debian:bookworm-slim@sha256:7ff6ba31527b9e336162fa061746c366c4b2032f4d2cff2078bea43f78efe3f8`
- `ubuntu:22.04@sha256:e0f80c829509ef4df9e76bd8944eb7b8ec25cbb379b112a7f9448b2c77a77c9b`

Highest required GLIBC symbol: **2.35**. Both tested glibc versions work; older glibc compatibility is not established. Native dependencies are system libc, libm, libstdc++ and libgcc_s plus the platform loader; there is no ONNX Runtime shared-library dependency. The generated dist Linux artifacts may have a different glibc baseline because their build runners differ; only the two explicitly tested containers are accepted timing evidence here.

The smoke exposed a preexisting requirement for Git during standalone `scan dir`. The CLI now permits directory scanning/redaction without Git only outside a repository and absent inherited Git routing. Repository contexts and Git scan modes still fail closed, avoiding silently dropping repository allow state or policy. Regression tests cover both cases.

### macOS: not yet measured

No macOS measurements were made here. `.github/workflows/install-smoke.yml` installs the published shell/PowerShell installers on Ubuntu, macOS and Windows, times installer-download-to-first-result, checks <=60 s and screenshot email bbox, and uploads timing JSON even on failure. It supports `release: published` and manual dispatch with a tag. GitHub releases created with GITHUB_TOKEN generally do not trigger downstream release workflows; manually dispatch the smoke workflow for such releases. No workflow was remotely triggered by this task.

## pre-commit integration

`hushpush hook pre-push --from-env` validates all six documented environment variables, maps them into a single native protocol line and invokes the existing scanner. Missing/invalid variables exit 2. New branches with zero FROM_REF and existing remote refs use the same semantics and exit codes as stdin; missing objects never trigger lazy fetch. `scan staged` intentionally scans the whole index: configure **pass_filenames: false** on both pre-commit stage entries; use always_run and require_serial.

The [current pre-commit docs](https://pre-commit.com/#pre-push) list the six variables. Its [implementation](https://github.com/pre-commit/pre-commit/blob/main/pre_commit/commands/hook_impl.py) selects the first eligible update and may omit FROM/TO for whole-history pushes. It cannot be represented honestly as a complete once-per-ref adapter. The native stdin hook remains the complete multi-ref path. `.pre-commit-hooks.yaml` defines `hushpush` (pre-commit stage, `scan staged`) and `hushpush-pre-push` (`hook pre-push --from-env`), both `language: system` so they run the installed release binary with embedded models. Checked with pre-commit 4.2.0 `try-repo` on a synthetic repo: a staged Luhn-valid test card blocks (exit 1) at both stages, clean input passes, and output shows only `[CARD len=16]`.

## Verification

- Windows `cargo fmt --all --check`: PASS.
- Windows `cargo clippy --locked --workspace --all-targets -- -D warnings`: PASS.
- Same clippy with `--features bundled-models`: PASS.
- Build with missing model directory: expected failure 101, clear named-file fetch hint.
- Build with corrupt model bytes: expected failure 101, SHA-256 mismatch diagnostic.
- Windows workspace tests (coordinator, MSVC 14.44): `cargo test --workspace --locked` 224 passed, 0 failed, 6 ignored (3 Unix-only tests account for Linux's 227); `-- --ignored` with `HUSHPUSH_MODELS` 6/6; with `bundled-models` and no `HUSHPUSH_MODELS` 6/6. Clippy `-D warnings` passes with default features and `bundled-models`.
- A4 standalone parser unit tests: 3 passed. Cargo check for CLI all targets: PASS.
- Root and fuzz deny: PASS with the approved `webpki-root-certs` exception.
- Windows `cargo xtask fuzz --docker --minutes 1 --target image_decode` (coordinator, MSVC 14.44): exit 0, 33,414 executions in 61 seconds, no crash. The fuzz workspace builds with the static runtime.


The fuzz run resolved additional download-feature dependencies transiently in fuzz/Cargo.lock. That out-of-scope file was restored byte-for-byte afterward; the resolved verification lock is local under target. The root lockfile contains the runtime dependency change. Corpus fixtures and labels are unchanged.

Linux verification used a synthetic copy on an ext4 Docker volume. Windows bind mounts failed preexisting no-clobber output rename semantics and are not used as Linux filesystem acceptance evidence. `cargo test --workspace --locked`: PASS, 227 passed and 6 ignored. With HUSHPUSH_MODELS, `cargo test --workspace --locked -- --ignored`: PASS, 6 passed. With bundled-models and no HUSHPUSH_MODELS: ignored suite PASS, 6 passed. Both new no-Git installation regression tests passed. Final environment parser focused tests: 3 passed after remote-name validation.

`cargo xtask eval --ci` with the statically linked runtime in Linux: **PASS (exit 0)**. All **158** baseline regression metrics are exactly unchanged, including every OCR metric: **zero deltas**, so no corpus, baseline or RESULTS regeneration is required. The preexisting OCR email@2x absolute recall gate remains FAIL at 0.9167 against 0.95, unchanged; the CI regression gate passes. Overall OCR recall at both 1x and 2x is 0.9722. Redaction retained all supported outputs, produced zero residual findings, and preserved inputs; its expected exit 3 covers the seven unsupported/incomplete fixtures. The generated 50,000,000-byte text scan took 1.4280 s, warm images mean/max 1.0968/1.5999 s with zero timing failures. These are host measurements, not real-scene accuracy claims.

Local evidence: `target/packaging-linux/eval.json`, `linux-eval.log`, `corpus-results-measured.md`, the separate test logs and per-container JSON timing/report files. Windows dist failure: `target/t14-dist-tool/windows-dist-build.log`. macOS and the two unbuilt aarch64/x86_64-apple variants have no local measurements; the Windows VC++ prerequisite is documented rather than removed.
