#!/usr/bin/env bash
# Source this script so the caller stays in the throwaway repository.
set -eu
demo_source="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
demo_temp="$(mktemp -d "${TMPDIR:-/tmp}/hushpush-demo.XXXXXX")"
git init --bare -q "$demo_temp/remote.git"
git init -q -b main "$demo_temp/repo"
cd "$demo_temp/repo"
git config user.name 'Synthetic Demo'
git config user.email 'demo@example.com'
git -c core.hooksPath=/dev/null commit -q --allow-empty -m 'Synthetic baseline'
git remote add origin "$demo_temp/remote.git"
git -c core.hooksPath=/dev/null push -q -u origin main
mkdir evidence
cp "$demo_source/evidence/synthetic.log" evidence/device.log
cp "$demo_source/evidence/screenshot.png" evidence/screenshot.png
git add evidence
git -c core.hooksPath=/dev/null commit -q -m 'Add synthetic debugging evidence'
export HUSHPUSH_DEMO_ROOT="$demo_temp"
export PS1='$ '
