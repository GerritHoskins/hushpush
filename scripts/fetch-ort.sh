#!/usr/bin/env bash
# Usage: bash scripts/fetch-ort.sh [target [archive-directory]]
# ORT_ARCHIVE_DIR defaults to runner temp (or ~/.cache)/ort-archives.
# Tests only: ORT_PREFETCH_TEST_CACHE_ROOT replaces ort-sys's cache root;
# ORT_PREFETCH_TEST_URL replaces the download URL (HTTPS is still required).
# Neither override changes the expected hash or ort-sys's linking environment.
set -euo pipefail

die() {
  printf 'fetch-ort: %s\n' "$2" >&2
  exit "$1"
}

cd -- "${BASH_SOURCE[0]%/*}/.."
[[ $# -le 2 ]] || die 2 'usage: fetch-ort.sh [target [archive-directory]]'

host=''
while read -r key value; do
  [[ "$key" != host: ]] || host=$value
done < <(rustc -vV)
[[ -n "$host" ]] || die 1 'rustc -vV reported no host target'
target=${1:-$host}
[[ "$target" =~ ^[a-zA-Z0-9_-]+$ ]] || die 2 "invalid target triple: $target"

version=''
in_ort=false
while IFS= read -r line; do
  line=${line%$'\r'}
  if [[ "$line" == 'name = "ort-sys"' ]]; then
    in_ort=true
  elif $in_ort && [[ "$line" == version\ =\ \"* ]]; then
    version=${line#*\"}
    version=${version%%\"*}
    break
  fi
done < Cargo.lock
[[ -n "$version" ]] || die 1 'no ort-sys version in Cargo.lock'

cargo fetch --locked --quiet
cargo_home=${CARGO_HOME:-${HOME:?}/.cargo}
cargo_home=${cargo_home//\\//}
dist=''
for candidate in "$cargo_home"/registry/src/*/ort-sys-"$version"/dist.txt; do
  [[ ! -f "$candidate" ]] || { dist=$candidate; break; }
done
[[ -n "$dist" ]] || die 1 "no dist.txt for ort-sys $version under $cargo_home/registry/src"

url=''
hash=''
while IFS=$'\t' read -r features triple row_url row_hash; do
  if [[ "$features" == none && "$triple" == "$target" ]]; then
    url=$row_url
    hash=${row_hash%$'\r'}
    break
  fi
done < "$dist"
[[ -n "$url" && "$hash" =~ ^[A-F0-9]{64}$ ]] || die 1 "no valid 'none' row for $target in $dist"

curl_args=(--silent --fail --location --proto '=https' --proto-redir '=https'
  --connect-timeout 30 --max-time 600 --speed-limit 32768 --speed-time 30)
case "$host" in
  *-windows-*)
    cache_root=${LOCALAPPDATA:?}/ort.pyke.io
    cache_root=${cache_root//\\//}
    curl_args+=(--ssl-revoke-best-effort)
    ;;
  *-apple-darwin) cache_root=${HOME:?}/Library/Caches/ort.pyke.io ;;
  *-linux-*)
    # ort-sys intentionally appends no suffix to an absolute XDG_CACHE_HOME.
    if [[ ${XDG_CACHE_HOME:-} == /* ]]; then
      cache_root=$XDG_CACHE_HOME
    else
      cache_root=${HOME:?}/.cache/ort.pyke.io
    fi
    ;;
  *) die 1 "unsupported host for ort-sys's cache layout: $host" ;;
esac
cache_root=${ORT_PREFETCH_TEST_CACHE_ROOT:-$cache_root}
cache_root=${cache_root//\\//}
destination=$cache_root/dfbin/$target/$hash
if [[ -d "$destination/onnxruntime" ]]; then
  printf '%s\n' "$destination/onnxruntime"
  exit 0
fi

archive_dir=${2:-${ORT_ARCHIVE_DIR:-${RUNNER_TEMP:-${TMPDIR:-${HOME:?}/.cache}}/ort-archives}}
archive_dir=${archive_dir//\\//}
mkdir -p -- "$archive_dir" "${destination%/*}"
archive=$archive_dir/$target-$hash.tgz
partial=$archive.part.$$.$RANDOM
temporary=${destination%/*}/tmp.$$.${RANDOM}_$hash
cleanup() {
  rm -f -- "$partial"
  rm -rf -- "$temporary"
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

verify() {
  local actual
  if command -v sha256sum >/dev/null 2>&1; then
    actual=$(sha256sum < "$1")
  else
    actual=$(shasum -a 256 < "$1")
  fi
  actual=${actual%% *}
  printf '%s %s\n' "$1" "$actual"
  # Bash 3.2 on Apple's runners has no ${value^^} expansion.
  (shopt -s nocasematch; [[ "$actual" == "$hash" ]])
}

if [[ -f "$archive" ]] && ! verify "$archive"; then
  rm -f -- "$archive"
fi
if [[ ! -f "$archive" ]]; then
  backoff=5
  for attempt in 1 2 3 4; do
    printf 'attempt %s/4\n' "$attempt"
    if curl "${curl_args[@]}" \
      --write-out '%{size_download} bytes\n' \
      --output "$partial" "${ORT_PREFETCH_TEST_URL:-$url}" && verify "$partial"; then
      mv -f -- "$partial" "$archive"
      break
    fi
    rm -f -- "$partial"
    [[ "$attempt" != 4 ]] || die 1 "download failed or did not verify after 4 attempts: $target"
    printf 'backoff %ss\n' "$backoff"
    sleep "$backoff"
    backoff=$((backoff * 2))
  done
fi
printf '%s\n' "$archive"

mkdir -- "$temporary"
# stdin avoids GNU tar interpreting a Windows drive letter as a remote host.
tar -xzf - -C "$temporary" < "$archive"
[[ -d "$temporary/onnxruntime" ]] || die 1 "archive has no onnxruntime/ directory: $archive"
if [[ ! -d "$destination/onnxruntime" ]]; then
  # Portable mv may move inside a concurrent winner's directory instead of
  # failing the rename. Remove only our uniquely named staging directory.
  mv -- "$temporary" "$destination" 2>/dev/null || [[ -d "$destination/onnxruntime" ]]
  if [[ -d "$destination/${temporary##*/}" ]]; then
    rm -rf -- "${destination:?}/${temporary##*/}"
  fi
fi
[[ -d "$destination/onnxruntime" ]] || die 1 "extraction did not produce $destination/onnxruntime"
printf '%s\n' "$destination/onnxruntime"
