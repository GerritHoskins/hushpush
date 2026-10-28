#!/usr/bin/env bash
set -eu
source /vhs/demo/setup.sh
set +e
hushpush scan push --include-reserved --no-user-patterns > push.txt
result=$?
set -e
cat push.txt
test "$result" -eq 1
grep -q 'evidence/device.log' push.txt
grep -q 'evidence/screenshot.png' push.txt
! grep -qi 'full history\|approximate' push.txt
hushpush redact evidence --out reviewed --include-reserved --no-user-patterns
hushpush scan dir reviewed --include-reserved --no-user-patterns > rescan.txt
cat rescan.txt
grep -q 'No findings in supported categories.' rescan.txt
test "$(find reviewed -type f | wc -l)" -eq 2
cmp evidence/device.log "$demo_source/evidence/synthetic.log"
cmp evidence/screenshot.png "$demo_source/evidence/screenshot.png"
