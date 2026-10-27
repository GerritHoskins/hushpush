# S4 clipboard structural detection

Reproduce from repository root: `python spikes/s4-clipboard/score.py`.
Synthetic data only; no third-party Python packages. Generated seed: 4102026.
The generator uses 55 distinct look-alike templates and varied prose, punctuation,
non-ASCII Latin text, quoted values, loose values, and multiline history.
The static `holdout.txt` and byte labels in `holdout.labels.json` are hand-written
regressions: the coordinator's sixteen records, additional ecosystem look-alikes,
and separately authored positives. They are maintained independently of generator
payloads. This is a known regression holdout, not an unseen population estimate.

The Rust scorer invokes both shipped detectors. A TP requires an exact UTF-8
byte span, detector id/version, and severity. Partial spans or incorrect severity
count as FP and FN. Metrics are reported overall and by format/severity.
Block precision/recall gates apply per dataset and format/severity; Warn has a
precision target of >=0.80, with recall also reported honestly. A group with
no predicted or expected positives has its empty denominator treated as 1.0;
all reported positive groups below have nonempty support. Raw fixture contents
and detected values are never printed by scoring.

## Generated

247 records: 192 positives, 55 look-alikes; 72 multiline positives.

| Group | TP | FP | FN | Precision | Recall |
|---|---:|---:|---:|---:|---:|
| format/android | 48 | 0 | 0 | 1.0000 | 1.0000 |
| format/android/severity/Block | 48 | 0 | 0 | 1.0000 | 1.0000 |
| format/fields | 96 | 0 | 0 | 1.0000 | 1.0000 |
| format/fields/severity/Warn | 96 | 0 | 0 | 1.0000 | 1.0000 |
| format/history | 48 | 0 | 0 | 1.0000 | 1.0000 |
| format/history/severity/Block | 48 | 0 | 0 | 1.0000 | 1.0000 |
| overall | 192 | 0 | 0 | 1.0000 | 1.0000 |
| severity/Block | 96 | 0 | 0 | 1.0000 | 1.0000 |
| severity/Warn | 96 | 0 | 0 | 1.0000 | 1.0000 |
## Holdout

72 records: 23 positives, 49 look-alikes; 5 multiline positives.

| Group | TP | FP | FN | Precision | Recall |
|---|---:|---:|---:|---:|---:|
| format/android | 5 | 0 | 0 | 1.0000 | 1.0000 |
| format/android/severity/Block | 5 | 0 | 0 | 1.0000 | 1.0000 |
| format/fields | 13 | 0 | 0 | 1.0000 | 1.0000 |
| format/fields/severity/Warn | 13 | 0 | 0 | 1.0000 | 1.0000 |
| format/history | 5 | 0 | 0 | 1.0000 | 1.0000 |
| format/history/severity/Block | 5 | 0 | 0 | 1.0000 | 1.0000 |
| overall | 23 | 0 | 0 | 1.0000 | 1.0000 |
| severity/Block | 10 | 0 | 0 | 1.0000 | 1.0000 |
| severity/Warn | 13 | 0 | 0 | 1.0000 | 1.0000 |

Gate: Block precision and recall >=0.90 per dataset/format/severity, and Warn
precision target >=0.80: **PASS**.
`clipboard@2` is Block for Android text and timestamped history;
`clipboard_field@1` is Warn for quoted and eligible unquoted fields.

The YAML description regression remains labelled negative. A Warn on this
ambiguous description is an accepted known FP, not a Block gate failure;
this run excludes it because its value contains the whole word "clipboard".
All Warn false positives, including this accepted case if detected, remain in
reported precision counts. The second review's ten supplied regressions are
included in the holdout; they are no longer unseen data.

## Why the earlier result was misleading

The earlier recorded run reported 90 TP, 0 FP, 0 FN (precision/recall 1.0000),
with 30 positives and 30 look-alikes per format. That result measured a narrow
self-authored fixture contract: positives reused one payload template, while each
format repeated ten easy negative templates three times. Unique context prefixes
did not create new semantic coverage. It did not justify shipping ordinary
unquoted fields at Block. The coordinator's independent review found all five
positives but six false Blocks among eleven look-alikes: precision 5/11 = 45.45%,
recall 5/5 = 100%. These historical figures are the supplied review evidence,
not a rerun or a new measurement of the previous detector.

Current runs above measure the corrected contract and explicit regressions.
They do not establish real-world precision; T11's independent corpus remains
necessary to measure generalization.
