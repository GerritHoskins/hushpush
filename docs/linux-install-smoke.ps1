param([switch]$SkipBuild)
$ErrorActionPreference = 'Stop'
$repo = Split-Path $PSScriptRoot -Parent
$rust = 'rust@sha256:3914072ca0c3b8aad871db9169a651ccfce30cf58303e5d6f2db16d1d8a7e58f'
$runtimes = @(
    'debian@sha256:7ff6ba31527b9e336162fa061746c366c4b2032f4d2cff2078bea43f78efe3f8',
    'ubuntu@sha256:e0f80c829509ef4df9e76bd8944eb7b8ec25cbb379b112a7f9448b2c77a77c9b'
)
$output = Join-Path $repo 'target/packaging-linux'
New-Item -ItemType Directory -Force $output | Out-Null
if (-not $SkipBuild) {
    docker run --rm --platform linux/amd64 -v "${repo}:/work" -w /work -v hushpush-t14-cargo:/usr/local/cargo -v hushpush-t14-target:/target -e CARGO_TARGET_DIR=/target $rust cargo build --release --locked -p hushpush-cli --features bundled-models --target x86_64-unknown-linux-gnu
    if ($LASTEXITCODE -ne 0) { throw 'Linux bundled build failed' }
}
$package = @'
set -eu
mkdir -p /work/target/packaging-linux/package
cp /target/x86_64-unknown-linux-gnu/release/hushpush /work/target/packaging-linux/package/
strip /work/target/packaging-linux/package/hushpush
cp LICENSE-MIT LICENSE-APACHE README.md NOTICE ThirdPartyNotices /work/target/packaging-linux/package/
tar -czf /work/target/packaging-linux/hushpush-x86_64-unknown-linux-gnu.tar.gz -C /work/target/packaging-linux/package .
ldd /work/target/packaging-linux/package/hushpush
objdump -T /work/target/packaging-linux/package/hushpush | sed -n 's/.*(GLIBC_\([^)]*\)).*/\1/p' | sort -Vu | tail -1
stat -c '%n %s bytes' /work/target/packaging-linux/package/hushpush /work/target/packaging-linux/hushpush-x86_64-unknown-linux-gnu.tar.gz
'@
[Convert]::ToBase64String([Text.Encoding]::UTF8.GetBytes($package.Replace("`r", ''))) | docker run --rm --platform linux/amd64 -i -v "${repo}:/work" -w /work -v hushpush-t14-target:/target $rust sh -c 'base64 --ignore-garbage -d | sh'
if ($LASTEXITCODE -ne 0) { throw 'Linux archive creation failed' }
$smoke = @'
set -eu
mkdir /tmp/fresh
cd /tmp/fresh
getconf GNU_LIBC_VERSION
start=$(date +%s%N)
tar -xzf /input/hushpush-x86_64-unknown-linux-gnu.tar.gz
extracted=$(date +%s%N)
set +e
env -i PATH=/usr/bin:/bin HOME=/tmp ./hushpush scan dir /demo/evidence --include-reserved --json /out/report.json
code=$?
set -e
finished=$(date +%s%N)
printf '{"extraction_ns":%s,"scan_ns":%s,"total_ns":%s,"exit_code":%s}\n' "$((extracted-start))" "$((finished-extracted))" "$((finished-start))" "$code" > /out/timing.json
cat /out/timing.json
exit 0
'@
foreach ($runtime in $runtimes) {
    $name = ($runtime -split '@')[0]
    $runtimeOutput = Join-Path $output $name
    New-Item -ItemType Directory -Force $runtimeOutput | Out-Null
    [Convert]::ToBase64String([Text.Encoding]::UTF8.GetBytes($smoke.Replace("`r", ''))) | docker run --rm --platform linux/amd64 -i --network none -v "${output}:/input:ro" -v "${repo}/demo:/demo:ro" -v "${runtimeOutput}:/out" $runtime sh -c 'base64 --ignore-garbage -d | sh'
    if ($LASTEXITCODE -ne 0) { throw "$name container failed" }
    $timing = Get-Content (Join-Path $runtimeOutput 'timing.json') -Raw | ConvertFrom-Json
    if ($timing.exit_code -ne 1) { throw "$name scan did not report the planted blocking card" }
    $report = Get-Content (Join-Path $runtimeOutput 'report.json') -Raw | ConvertFrom-Json
    $boxes = @($report.findings | Where-Object { $_.category -eq 'email' -and $_.location.kind -eq 'bbox' })
    if ($boxes.Count -lt 1) { throw "$name screenshot email bbox missing" }
    if ($timing.total_ns -gt 60000000000) { throw "$name extraction-to-first-result exceeded 60 seconds" }
    Write-Host "$name PASS: screenshot email bbox; extraction-to-exit within 60 seconds"
}
