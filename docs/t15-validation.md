# T15 validation record

Measured 2026-10-28 at base `624054997bc051264d07ffffd6ea82ba6b2e7652`.
Synthetic fixtures only. No source-checkout commit, push, tag, release, hook
installation or publication was performed. The demo uses commits and a push
only inside a throwaway repository with a bare local remote.

## Quickstart runs

Reproduce with `./docs/verify-quickstart.ps1` from the clone root after T14's
Windows archive and Linux bundled target exist. The script executes the README
commands, validates exits and stages only the reviewed copy in isolated trial
repositories. It prints the native hook without installing it. Local logs are
under `target/t15/quickstart.log` (not product source).

| Check | Windows host | Linux container |
|---|---|---|
| Archive | Existing T14 Windows x64 ZIP | Local xz wrapper around T14-built Linux x86_64 binary |
| Version | 0.1.0, embedded models | 0.1.0, embedded models |
| Extraction + version + directory + HTTPS sample download + first scan | **1.6148812 s** | **1.441740982 s** |
| Initial scan | exit **1**, masked email and card | exit **1**, same masked findings |
| Redact | exit **0**, one retained copy | exit **0**, one retained copy |
| Review | Both replacement markers present | Same |
| Rescan | exit **0**, supported-category wording | Same |
| Stage | Only reviewed log, three insertions | Same |
| Hook print | exit **0**, shell script printed | Same |

Timing starts with the archive and prerequisites already available. It includes
extraction and the real pinned raw.githubusercontent.com sample download; it
excludes human reading/review time. The earlier already-installed Windows run
took 0.2999446 s from version to first result. These are local measurements,
not fresh-VM acceptance or a guarantee about another network/host.

The Linux runtime is T14's
`ubuntu@sha256:e0f80c829509ef4df9e76bd8944eb7b8ec25cbb379b112a7f9448b2c77a77c9b`,
reporting glibc 2.35. This minimal base needed curl, CA certificates, Git and
xz-utils installed before the timer. Windows already had the required VC++
runtime. Windows Git emitted an LF-to-CRLF warning when staging; staging still
returned 0. Windows PowerShell 5.1 decorates native stderr as `NativeCommandError`
in redirected logs, including benign package-tool notices; actual process
exits and asserted outcomes determine the results above.

README excerpts were copied from these runs: `[EMAIL len=16]`, `[CARD len=19]`,
the retained-copy line, replacement markers, and final supported-category line.
The initial single-file `scan dir` experiment returned exit 2 (directory
unavailable); the documented flow correctly supplies a directory. No CLI change
was made or needed. Public installers cannot be run successfully before the
first release, so the README explicitly uses owner-supplied pre-release archives.
Optional framework hook installation was not executed, per the no-hooks boundary.

## Preserved reference material

| Former README section | New home |
|---|---|
| Detectors and external patterns | [detectors.md](detectors.md) |
| Reports, exit codes, config, fingerprints and allows | [reference.md](reference.md) |
| Independent corpus evaluation, baseline and sealed split | [evaluation.md](evaluation.md) |
| Detailed limitations and security boundaries | [limitations.md](limitations.md) |
| Source builds, model lookup, future installer entry points | [building.md](building.md) |

The four complete reference sections were checked against `git show HEAD:README.md`:
their original bodies are preserved, apart from rebased relative links. The source
build/model paragraph is also preserved. README whitespace word count: **2,771 →
1,070**, including the demo alt text. `python docs/check-links.py` checks local
targets and heading anchors in README, CONTRIBUTING, the demo guide and all new
Markdown docs. It does not require a network or inspect personal files.

## Demo

See [demo reproduction](../demo/README.md) for the pinned VHS release/image digest,
Linux build contract and visual-review procedure. The full asserted tape flow
found log and screenshot findings without a full-history/approximation warning,
wrote two redacted copies and rescanned with exit 0. Input hashes were unchanged.
The tape deliberately disables shell errexit after setup because the expected
first scan returns 1.

Final GIF: **19.12 seconds**, **341,108 bytes** (below 3 MB), 1280×800 pixels,
22 px font, 10 fps, no typing delay. SHA-256:
`94ba52ee017e422ad8b91f65441c160c687428094a7c314b5ae5fd672de02ea1`.
VHS image index digest:
`sha256:ea49a6a1c529be83153e88321892b5585964418f0b9055e8c1e0d732194234a3`.
Final PNG frames at 4, 10 and 17 seconds were opened and visually inspected:
masked log/card and screenshot-box findings, two-copy success, and final
supported-category wording were readable; no raw detected value was visible.
The final embed references this inspected GIF.

Protected evidence SHA-256 values (before and after):

- `demo/evidence/screenshot.png`: `ec065081c0108cbfc90ba28f628a68db513ac4399789928b6285b1d3a5cfa579`
- `demo/evidence/synthetic.log`: `0a07ea2dc2964c9697901eae87a9961d09d59109d99e5ed748c57709cd609876`

## Checks and acceptance

| Acceptance item | Status / evidence |
|---|---|
| A1: No clone assumed; same evidence path; one pinned sample download command per OS | **met** |
| A2: Actual masked output/exits; expected 1 and reserved data explained | **met** |
| A3: Install/version/scan/redact/review/manual stage/optional hook flow | **met** for owner-supplied archives on Windows/Linux |
| Public release install and macOS quickstart | **not verified**; no release exists; no macOS environment |
| A4: Privacy companion positioning; no certification claim | **met** |
| A5: All former limitations retained; prerequisites and framework restrictions added | **met** |
| A6: Shorter README; reference facts retained; relative links/anchors resolve | **met** |
| B: Privacy rules, setup, required checks, four code-derived entry points, licensing, shape-only security reports | **met**; named paths checked |
| C: Outside-tester rubric, criteria, times, data-minimal reporting | **met** |
| T15: Three outside testers succeed unaided | **not verified**; owner must recruit them |
| D: Temp repo, local tracking remote, one outgoing synthetic evidence commit | **met** |
| D: Digest-pinned VHS ≥two weeks old; T14-style Linux bundled build; real GIF ≤3 MB | **met** |
| D: Full flow, no raw-value terminal display, PNG-frame inspection before final embed | **met** |
| `cargo fmt --check` | **met**, exit 0 |
| `cargo clippy --all-targets -- -D warnings` | **met**, exit 0 |
| `cargo test` | **met**, exit 0; model-dependent ignored tests remain ignored |
| `cargo deny check` | **met**, exit 0; advisories/bans/licenses/sources OK |
| Only owned paths changed; both evidence files byte-identical | **met**; preexisting `.gitguardian.yaml` left untouched |
| No commit/push/tag/release/hook install in source checkout | **met** |

Cargo outputs: `target/t15/cargo-test.log`, `clippy.log`, `cargo-deny.log`.
Render/flow output: `target/t15/render.log`. Generated local logs and extracted
frames are intentionally untracked; only the demo GIF is a deliverable.
