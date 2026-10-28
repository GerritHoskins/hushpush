# Verify the README commands using pre-release archives, without installing hooks.
$ErrorActionPreference = 'Continue'
$repo = Split-Path $PSScriptRoot -Parent
$out = Join-Path $repo 'target/t15'
$trial = Join-Path $out ('quickstart-' + (Get-Date -Format 'yyyyMMdd-HHmmss'))
New-Item -ItemType Directory -Force $trial -ErrorAction Stop | Out-Null
Copy-Item (Join-Path $repo 'target/distrib/hushpush-cli-x86_64-pc-windows-msvc.zip') $trial -ErrorAction Stop
$oldPath = $env:PATH
$oldCeiling = $env:GIT_CEILING_DIRECTORIES
$env:GIT_CEILING_DIRECTORIES = $out
Push-Location $trial
try {
    $timer = [Diagnostics.Stopwatch]::StartNew()
    Expand-Archive ./hushpush-cli-x86_64-pc-windows-msvc.zip -DestinationPath ./hushpush-bin
    $env:PATH = (Resolve-Path ./hushpush-bin).Path + ';' + $env:PATH
    hushpush --version
    if ($LASTEXITCODE -ne 0) { throw 'Version failed' }
    mkdir evidence | Out-Null
    Invoke-WebRequest https://raw.githubusercontent.com/GerritHoskins/hushpush/624054997bc051264d07ffffd6ea82ba6b2e7652/demo/evidence/synthetic.log -OutFile evidence/synthetic.log -ErrorAction Stop
    hushpush scan dir ./evidence --include-reserved --no-user-patterns
    $scanExit = $LASTEXITCODE
    $firstResult = $timer.Elapsed.TotalSeconds
    Write-Output "scan_exit=$scanExit extraction_to_first_result_seconds=$firstResult"
    if ($scanExit -ne 1 -or $firstResult -gt 60) { throw 'First-result criterion failed' }
    hushpush redact ./evidence --out ./evidence-redacted --include-reserved --no-user-patterns
    if ($LASTEXITCODE -ne 0) { throw 'Redaction failed' }
    Get-Content ./evidence-redacted/synthetic.log
    hushpush scan dir ./evidence-redacted --include-reserved --no-user-patterns
    if ($LASTEXITCODE -ne 0) { throw 'Rescan failed' }
    git init
    if ($LASTEXITCODE -ne 0) { throw 'Trial Git init failed' }
    git add ./evidence-redacted/synthetic.log
    if ($LASTEXITCODE -ne 0) { throw 'Stage failed' }
    git diff --cached --stat
    hushpush hook print
    if ($LASTEXITCODE -ne 0) { throw 'Hook print failed' }
} finally {
    Pop-Location
    $env:PATH = $oldPath
    $env:GIT_CEILING_DIRECTORIES = $oldCeiling
}
$rust = 'rust@sha256:3914072ca0c3b8aad871db9169a651ccfce30cf58303e5d6f2db16d1d8a7e58f'
# Wrap the T14-built Linux binary in the README's release-style archive layout.
docker run --rm --platform linux/amd64 -v "${out}:/out" -v hushpush-t14-target:/target:ro $rust tar -cJf /out/hushpush-cli-x86_64-unknown-linux-gnu.tar.xz -C /target/x86_64-unknown-linux-gnu/release hushpush
if ($LASTEXITCODE -ne 0) { throw 'Linux trial archive failed' }
$linux = @'
set -eu
# curl is a download prerequisite; this minimal Ubuntu base does not ship it.
apt-get update -qq
apt-get install -y -qq --no-install-recommends curl ca-certificates git xz-utils >/dev/null
mkdir /tmp/trial
cd /tmp/trial
cp /out/hushpush-cli-x86_64-unknown-linux-gnu.tar.xz .
getconf GNU_LIBC_VERSION
start=$(date +%s%N)
mkdir hushpush-bin
tar -xJf hushpush-cli-x86_64-unknown-linux-gnu.tar.xz -C hushpush-bin
export PATH="$PWD/hushpush-bin:$PATH"
hushpush --version
mkdir evidence
curl --proto '=https' --tlsv1.2 -fsSL https://raw.githubusercontent.com/GerritHoskins/hushpush/624054997bc051264d07ffffd6ea82ba6b2e7652/demo/evidence/synthetic.log -o evidence/synthetic.log
set +e
hushpush scan dir ./evidence --include-reserved --no-user-patterns
code=$?
set -e
finished=$(date +%s%N)
printf 'scan_exit=%s extraction_to_first_result_ns=%s\n' "$code" "$((finished-start))"
test "$code" -eq 1
test "$((finished-start))" -le 60000000000
hushpush redact ./evidence --out ./evidence-redacted --include-reserved --no-user-patterns
cat ./evidence-redacted/synthetic.log
hushpush scan dir ./evidence-redacted --include-reserved --no-user-patterns
git init
git add ./evidence-redacted/synthetic.log
git diff --cached --stat
hushpush hook print
'@
[Convert]::ToBase64String([Text.Encoding]::UTF8.GetBytes($linux.Replace("`r", ''))) | docker run --rm --platform linux/amd64 -i -v "${out}:/out:ro" 'ubuntu@sha256:e0f80c829509ef4df9e76bd8944eb7b8ec25cbb379b112a7f9448b2c77a77c9b' sh -c 'base64 --ignore-garbage -d | sh'
if ($LASTEXITCODE -ne 0) { throw 'Linux quickstart failed' }
