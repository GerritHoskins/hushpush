# Outside-tester guide

Owner: recruit three developers who have not used hushpush. Supply only the
README link and an owner-built bundled-model archive for their OS until a public
release exists. Tell them which prerequisites must already be installed. Give
no coaching, extra commands, repository clone, sample files, or corrections.
Each tester follows the [README](../README.md) alone. This guide is the owner's
observation/report rubric, not an alternate quickstart.

Start a timer when the tester has the archive and prerequisites. Record elapsed
seconds at the first result; stop timing other steps individually. Public
installer success and fresh-machine prerequisites need separate release checks.
Use only the pinned synthetic sample. Never request their own logs or images.

| README step | Success criterion | Pass / fail | Time (seconds) |
|---|---|---|---|
| Install / version | Archive unpacks; version shows embedded models, exit 0 | | |
| Download | One synthetic log in `evidence/`, no clone needed | | |
| First scan | Masked EMAIL/CARD; expected exit 1 understood; first result within 60 seconds of starting | | |
| Redact | New copy, exit 0; original untouched | | |
| Review / rescan | Tester locates markers, reviews copy; rescan exit 0 with supported-category wording | | |
| Stage | Tester stages only the reviewed copy in their trial Git repository | | |
| Optional hooks | Tester understands native all-ref vs framework first-ref limits; installation is optional | | |

Report **only** a tester alias, OS/architecture, hushpush version, pass/fail,
elapsed times, whether assistance was needed, and confusion points using command
or step names. Do not attach terminal output, reports, screenshots, repositories,
files, paths, or any personal data. A shape-only explanation is enough.

```text
Tester alias:
OS / architecture / hushpush version:
Prerequisites present: pass/fail
Install/version: pass/fail; seconds:
Download: pass/fail; seconds:
First scan: pass/fail; elapsed seconds since start:
Redact: pass/fail; seconds:
Review/rescan: pass/fail; seconds:
Stage reviewed copy: pass/fail; seconds:
Optional hooks: understood / skipped / confused
Assistance needed: yes/no
Confusion points (step names only; no data):
```

Acceptance requires **three independent testers** completing the required steps
unaided, with first results within 60 seconds after prerequisites/archive are
available. Report each attempt, including failures; revise the README and recruit
fresh testers if coaching was necessary. Hook installation is not required.
Current T15 acceptance: **not verified**; automated runs are not outside testers.
