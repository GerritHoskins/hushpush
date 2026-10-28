# Reproduce the synthetic demo

From the repository root in PowerShell, with Docker's Linux engine running:

```powershell
./demo/render.ps1
```

The script builds `hushpush-cli` for **Linux x86_64** with `bundled-models`,
`--release --locked`, the same digest-pinned Rust builder, named caches and
target used by [T14's smoke script](../docs/linux-install-smoke.ps1). Verified
models must already exist (`cargo xtask fetch-models`); no scan downloads them.
`-SkipBuild` reuses that target only when it has already been built this way.

The [Dockerfile](Dockerfile) pins
`ghcr.io/charmbracelet/vhs@sha256:ea49a6a1c529be83153e88321892b5585964418f0b9055e8c1e0d732194234a3`.
This is [VHS v0.12.1](https://github.com/charmbracelet/vhs/releases/tag/v0.12.1),
published **2026-09-24**, more than two weeks before the 2026-10-28 run.
The Linux amd64 child manifest is
`sha256:3afed459e6d7860f6bb8b9a22ae29d5927d628dac6e30f32a250113406a348b9`.
The base image lacks Git; the tooling layer installs Git and CA certificates
from its configured distribution repositories. Those package repositories
are not frozen, so the script reproduces the procedure rather than promising
bit-identical GIF bytes. Rendering runs with `--network none`.

The [setup script](setup.sh) creates a temporary repository and a bare **local**
remote. An empty baseline is sent only to that local remote, creating tracking
refs. One outgoing commit adds the unchanged synthetic log and screenshot from
[`evidence/`](evidence/). There is no full-history warning. Nothing is committed
or pushed in the source checkout, and no hook is installed. When sourced
manually, `HUSHPUSH_DEMO_ROOT` identifies the temporary parent; the render
container discards it automatically.

The [flow check](verify-flow.sh) asserts exit 1, both artifact paths, no
full-history/approximation warning, two retained copies, unchanged input bytes,
and an exit-0 rescan. The fixture intentionally produces **three findings**:
two in the log (email and sandbox card), one email box in the screenshot.
The [tape](demo.tape) shows these real findings, then redaction and rescan.
It displays neither original log contents nor screenshot pixels. There is no
image upload or publication step.

`render.ps1` writes `demo.gif`, enforces a 3,000,000-byte maximum, prints measured
duration/size, and extracts PNG frames under `target/t15/` at 4, 10 and 17 seconds.
Open **all three** before embedding a new render: only masked previews may be
visible. See [T15 validation](../docs/t15-validation.md) for this run's measurements,
GIF hash and acceptance checklist.
