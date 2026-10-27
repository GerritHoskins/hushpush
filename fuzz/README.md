# Local fuzzing

This is an independent Cargo workspace, excluded from the product workspace.
Its unstable core entry points exist only with the `fuzzing` feature. Harnesses
never initialize OCR or read files outside their supplied synthetic inputs.

Run `cargo xtask fuzz --docker --minutes 2` from the repository root, or
`bash scripts/fuzz-docker.sh --minutes 2 --target image_decode`.
Minutes apply to each selected target; build and dependency installation are extra.
The coordinator runs the one-hour campaign separately.

The runner uses nightly Rust image `rustlang/rust`, pulled on 2026-10-27, pinned to
`sha256:7fcf69d30fc42bd6f652305d904cfefa8818377401ca912717fe935953ff9711`.
It installs `cargo-fuzz` **0.13.2** with its locked dependencies. The named Docker
volume `hushpush-fuzz-target` contains Linux compilation output, downloaded crates,
mutated corpora and temporary artifacts; Windows target output is never reused.
Each target gets `-max_total_time=<minutes*60>` and `-rss_limit_mb=2048`.
Crashes return a failing status and are copied to `fuzz/artifacts/<target>/`;
logs (including actual execution counts) are in `fuzz/logs/<target>.log`.
Both directories are ignored by Git. Checked-in seeds are immutable during runs.

Targets cover classification/text extraction/all text detectors/locations,
limited image decoding without OCR, both TOML parsers, all three Git parsers,
and text redaction with value-removal and clean-rescan assertions. Seeds are
synthetic; selected corpus fixtures are copied byte-for-byte, never modified.

## Smoke evidence (2026-10-27)

The Docker smoke runs use `-max_total_time=120`; libFuzzer rounds completion to
121 elapsed seconds. These are smoke measurements, not the coordinator's
one-hour campaign. Mutated corpora persist in the named volume between runs.

| Target | Executions | Outcome |
|---|---:|---|
| classify_decode_detect | 40,546 | PASS |
| image_decode | 102,008 | PASS |
| config_parse | 516,833 | PASS |
| git_parsers | 2,251,138 | PASS |
| redact_text | 61,506 | PASS |

The first classification attempt exposed an empty-input harness initialization
error (the synthetic pattern omitted its required severity). A regression test
now covers initialization. The redaction target found a substitution that
created a new detector match. `cargo fuzz tmin` reduced the 165-byte crash to
14 bytes; the exact minimized synthetic input is retained as a seed and a
feature-enabled regression test. The product now checks redacted output with
the effective detector configuration and uses opaque output when replacement
markers would retain or create a finding. No detector or corpus label was loosened.

The coordinator approved the NCSA license required by `libfuzzer-sys` only for
this independent fuzz workspace. `fuzz/deny.toml` repeats the shipped dependency
policy and adds NCSA; the root `deny.toml` remains unchanged. Audit separately:
`cargo deny --manifest-path fuzz/Cargo.toml check` (discovers `fuzz/deny.toml`).
