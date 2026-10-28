param([switch]$SkipBuild)
# Windows PowerShell 5.1 treats native stderr as an error when redirected.
# Every native command below checks its process exit code explicitly.
$ErrorActionPreference = 'Continue'
$repo = Split-Path $PSScriptRoot -Parent
$rust = 'rust@sha256:3914072ca0c3b8aad871db9169a651ccfce30cf58303e5d6f2db16d1d8a7e58f'
$output = Join-Path $repo 'target/t15'
New-Item -ItemType Directory -Force $output -ErrorAction Stop | Out-Null
if (-not $SkipBuild) {
    # Same pinned builder, target, feature, lockfile and caches as T14.
    docker run --rm --platform linux/amd64 -v "${repo}:/work" -w /work -v hushpush-t14-cargo:/usr/local/cargo -v hushpush-t14-target:/target -e CARGO_TARGET_DIR=/target $rust cargo build --release --locked -p hushpush-cli --features bundled-models --target x86_64-unknown-linux-gnu
    if ($LASTEXITCODE -ne 0) { throw 'Linux bundled build failed' }
}
docker build --platform linux/amd64 -t hushpush-t15-vhs -f "$PSScriptRoot/Dockerfile" $PSScriptRoot
if ($LASTEXITCODE -ne 0) { throw 'VHS tooling image failed' }
$render = @'
set -eu
cp /target/x86_64-unknown-linux-gnu/release/hushpush /usr/local/bin/hushpush
hushpush --version
# Assert the exact flow before recording; stdout remains shape-only.
bash /vhs/demo/verify-flow.sh
vhs /vhs/demo/demo.tape
test "$(stat -c %s /vhs/demo/demo.gif)" -le 3000000
ffprobe -v error -show_entries format=duration,size -of json /vhs/demo/demo.gif
ffmpeg -loglevel error -y -ss 4 -i /vhs/demo/demo.gif -frames:v 1 /vhs/target/t15/demo-scan.png
ffmpeg -loglevel error -y -ss 10 -i /vhs/demo/demo.gif -frames:v 1 /vhs/target/t15/demo-redact.png
ffmpeg -loglevel error -y -ss 17 -i /vhs/demo/demo.gif -frames:v 1 /vhs/target/t15/demo-rescan.png
'@
[Convert]::ToBase64String([Text.Encoding]::UTF8.GetBytes($render.Replace("`r", ''))) | docker run --rm --platform linux/amd64 --network none -i --entrypoint bash -v "${repo}:/vhs" -v hushpush-t14-target:/target:ro hushpush-t15-vhs -c 'base64 --ignore-garbage -d | bash'
if ($LASTEXITCODE -ne 0) { throw 'VHS render or flow verification failed' }
Write-Host 'Inspect target/t15/demo-{scan,redact,rescan}.png before embedding the GIF.'
