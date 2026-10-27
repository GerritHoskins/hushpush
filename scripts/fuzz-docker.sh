#!/usr/bin/env bash
set -euo pipefail
image='rustlang/rust@sha256:7fcf69d30fc42bd6f652305d904cfefa8818377401ca912717fe935953ff9711'
targets=(classify_decode_detect image_decode config_parse git_parsers redact_text)
inside=false
minutes=''
selected=''
while (($#)); do
  case "$1" in
    --inside) inside=true; shift ;;
    --minutes) minutes="${2:?missing minutes}"; shift 2 ;;
    --target) selected="${2:?missing target}"; shift 2 ;;
    *) echo 'usage: scripts/fuzz-docker.sh --minutes N [--target T]' >&2; exit 2 ;;
  esac
done
[[ "$minutes" =~ ^[1-9][0-9]*$ ]] || { echo 'minutes must be positive' >&2; exit 2; }
if [[ -n "$selected" ]]; then
  valid=false
  for target in "${targets[@]}"; do [[ "$selected" != "$target" ]] || valid=true; done
  $valid || { echo 'unknown fuzz target' >&2; exit 2; }
  targets=("$selected")
fi
if ! $inside; then
  root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
  if command -v cygpath >/dev/null 2>&1; then root="$(cygpath -w "$root")"; fi
  args=(--inside --minutes "$minutes")
  [[ -z "$selected" ]] || args+=(--target "$selected")
  # MSYS otherwise rewrites Linux paths passed to Docker Desktop.
  export MSYS_NO_PATHCONV=1
  exec docker run --rm --mount "type=bind,source=$root,target=/repo" \
    --mount type=volume,source=hushpush-fuzz-target,target=/fuzz-target \
    -w /repo "$image" bash /repo/scripts/fuzz-docker.sh "${args[@]}"
fi
export CARGO_TARGET_DIR=/fuzz-target/build
export CARGO_HOME=/fuzz-target/cargo-home
export PATH="$CARGO_HOME/bin:$PATH"
mkdir -p "$CARGO_HOME" /repo/fuzz/logs
if [[ ! -f "$CARGO_HOME/.hushpush-cargo-fuzz-0.13.2" ]]; then
  cargo install cargo-fuzz --version 0.13.2 --locked
  touch "$CARGO_HOME/.hushpush-cargo-fuzz-0.13.2"
fi
status=0
cd /repo/fuzz
for target in "${targets[@]}"; do
  mkdir -p "/fuzz-target/corpus/$target" "/fuzz-target/artifacts/$target" "artifacts/$target"
  cp -a "corpus/$target/." "/fuzz-target/corpus/$target/"
  if ! cargo fuzz run "$target" "/fuzz-target/corpus/$target" -- \
    "-max_total_time=$((minutes * 60))" -rss_limit_mb=2048 \
    "-artifact_prefix=/fuzz-target/artifacts/$target/" 2>&1 | \
    awk '/^#/ && /INITED|pulse|DONE/ {sub(/ MS:.*/, ""); print; fflush(); next} /^INFO:|^Done [0-9]+ runs|^==[0-9]+== ERROR:|^Error: Fuzz target exited/ {print; fflush()}' | \
    tee "logs/$target.log"; then
    status=1
    echo "Fuzz target $target failed; inspect its crash artifact." >&2
  fi
  cp -a "/fuzz-target/artifacts/$target/." "artifacts/$target/"
done
exit "$status"
