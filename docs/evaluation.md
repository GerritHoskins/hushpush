# Independent corpus evaluation


The seeded, synthetic [`corpus/`](../corpus/) is authored from the public contract independently of detector implementations. Its labels record planted byte spans and browser glyph boxes; screenshot PNGs and truth are checked in, so evaluation needs no browser. See the generated [`corpus/RESULTS.md`](../corpus/RESULTS.md) for measured precision, recall, OCR localization, phone recall at 1x/2x, coverage, redaction and performance gates. These measurements describe this corpus, not real-world accuracy.

Run from the repository root with verified local OCR assets:

```bash
cargo xtask fetch-models
cargo build --release --locked
HUSHPUSH_MODELS="$PWD/models" cargo xtask eval
```

The harness runs the release CLI with reserved synthetic values included and the corpus user patterns loaded. It writes `target/eval.json` and regenerates `corpus/RESULTS.md`. Inputs are checked for changes during the redaction evaluation; the 50 MB throughput fixture is generated temporarily.

When `HUSHPUSH_MODELS` is absent, a set `LEAKLENS_MODELS` is ignored with a one-line hint to use `HUSHPUSH_MODELS`.

To deliberately record a reviewed baseline, run `cargo xtask eval --split dev --write-baseline` with `HUSHPUSH_MODELS` set. This generates `corpus/baseline.json`; ordinary evaluations do not update it. CI runs `cargo xtask eval --split dev --ci` and fails on a metric falling more than one percentage point below that baseline. Absolute gates remain visible as PASS/FAIL, but known gate failures alone do not fail CI. Review failures as detector evidence; do not change the corpus to make a gate pass.

The documented clipboard field keys are `clipboard`, `pasteboard`, `clip_text`,
`primaryClip`, and `copiedText`.

Labels separate contract `core` bytes from whole-record `extent` diagnostics.
Text matching requires at least 90% core coverage; spill is reported separately.
Off-spec national phones without country codes and undocumented clipboard keys
are tracked separately and excluded from gates. The sealed `corpus/test/` split
(seed `20261028`) is for milestone checks only: `cargo xtask eval --split test`.
Its manifest hash is pinned in the dev baseline; CI and baseline writes use dev
only. Test reports preserve the dev reports.

Earlier detector-adjacent measurements remain in [`xtask/RESULTS.md`](../xtask/RESULTS.md) and [`spikes/s4-clipboard/RESULTS.md`](../spikes/s4-clipboard/RESULTS.md).
