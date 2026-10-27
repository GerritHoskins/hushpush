> Renamed to hushpush on 2026-10-27; historical prose and results below are unchanged.

# S1b: PP-OCRv5 through Rust ort

Uses only the existing synthetic S1 corpus. Do not regenerate it or point these scripts at personal files. All commands below run from `spikes/s1b-ppocr` in PowerShell, with Python 3.10+, Node, Rust stable, and MSVC build tools installed.

```powershell
python -m venv .venv
.venv/Scripts/python.exe -m pip install -r oracle-requirements.txt
.venv/Scripts/python.exe fetch-models.py
.venv/Scripts/python.exe fetch-runtime.py
& "$env:USERPROFILE/.cargo/bin/cargo.exe" build --release --locked --manifest-path runner/Cargo.toml
.venv/Scripts/python.exe run.py mobile server
.venv/Scripts/python.exe oracle.py --variant mobile
.venv/Scripts/python.exe oracle.py --variant server
.venv/Scripts/python.exe report.py
```

The independent runner Cargo workspace and lockfile do not use the root workspace. `run.py` verifies every PNG and truth SHA-256 against S1's existing manifest, selects the official local CPU runtime via `ORT_DYLIB_PATH`, then runs both variants sequentially. It needs S1 `out/manifest.json`, images, truth and `scores.json` already present. It neither generates nor changes S1 artifacts. `fetch-models.py` pins official PaddlePaddle repository revisions; dictionary entries come from each recognizer's own `inference.yml`, preserving order. `provenance.json` records model/dictionary URLs, sizes, SHA-256 and explicit model-card license statements. Model cards and configurations are saved alongside ignored models. Python packages are the correctness oracle and reproduction utilities; inference measurements use Rust only.

For mobile only: `.venv/Scripts/python.exe run.py mobile`, then `node score.mjs mobile`. `report.py` includes variants present in the latest scoring output. Scoring never prints recognized values; raw synthetic OCR strings are isolated under ignored `out/`. `Sensitive<T>` redacts Rust formatting; explicit raw JSON is the requested synthetic spike artifact.

Direct runner syntax:

```powershell
$env:ORT_DYLIB_PATH = (Resolve-Path models/runtime/onnxruntime.dll).Path
./runner/target/release/s1b-ppocr-runner.exe models ../s1-ocr/out out/mobile mobile 4 960
```

Optional trailing image limit supports smoke runs; omit it for scoring all images. Sessions load once, then the first image warms up once. PNG decode is outside timing; det resize/normalize, inference, DB geometry, perspective crops, rec preprocessing/inference and CTC geometry are inside timing. JSON serialization is excluded. Four intra-op threads and one inter-op thread; CPU provider only. The S1 default-thread desktop baseline is a comparison, not a controlled equal-thread benchmark. No laptop measurements are claimed.

The `ort` crate is pinned at 2.0.0-rc.10, using `std` and `load-dynamic`. `fetch-runtime.py` downloads Microsoft's CPU-only ONNX Runtime 1.22.0 Windows x64 ZIP and extracts its DLLs/license/notices locally. It does not alter system runtimes. The preliminary `download-binaries` attempt and its archive are documented in `runtime-provenance.json`; the actual DLL used is documented in `runtime-provenance-official.json`. No runtime downloads occur in Rust. The official ZIP contains debugging/build artifacts beyond the DLLs and is not a proposed release bundle.

Network isolation was not exercised: the current process token is not Administrator, and the running Windows Firewall service requires elevation for blocking rules. No firewall or adapter settings were changed. Local model/runtime resolution and absence of downloader features are structural evidence only; they do not replace a blocked-network run.

Quality checks, from this directory:

Formatting, Clippy, the seven scorer/corpus tests and three Rust tests pass. License/source checks pass. Full `cargo deny check` currently fails on dependency advisories; RESULTS records the affected crates and advisory IDs. No advisories are suppressed and no dependency updates were made.

```powershell
node --test test.mjs
& "$env:USERPROFILE/.cargo/bin/cargo.exe" fmt --check --manifest-path runner/Cargo.toml
& "$env:USERPROFILE/.cargo/bin/cargo.exe" clippy --locked --manifest-path runner/Cargo.toml --all-targets -- -D warnings
& "$env:USERPROFILE/.cargo/bin/cargo.exe" test --locked --manifest-path runner/Cargo.toml
Push-Location runner
& "$env:USERPROFILE/.cargo/bin/cargo.exe" deny check
Pop-Location
```

`RESULTS.md` is generated from measured `out/` evidence, checked provenance and recorded validation. Per-image OCR polygons are in original device pixels. `words` holds the whole line as a compatibility entry; it is not a measured word box. `chars` records decoded CTC token ranges projected back through the inverse crop transform. These are approximate localization ranges, not supervised character boxes. Scoring compares the identical reconstructed text in line and char modes, using S1 regex/Luhn rules without character correction. See RESULTS for implementation differences from the Python oracle and their measured impact.
