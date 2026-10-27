"""Score generated corpus and static holdout; emit aggregate metrics only."""
import json
import os
from pathlib import Path
import subprocess
from generate import ROOT, SEED, NEGATIVES, generate

repo = ROOT.parent.parent
cargo = Path(os.environ.get("USERPROFILE", str(Path.home()))) / ".cargo" / "bin" / "cargo.exe"
command = str(cargo) if cargo.exists() else "cargo"
corpus, generated = generate()
holdout_bytes = (ROOT / "holdout.txt").read_bytes()
holdout = []
for label in json.loads((ROOT / "holdout.labels.json").read_text(encoding="utf-8")):
    text = holdout_bytes[label["start"]:label["end"]].decode("utf-8")
    for span in label["spans"]:
        assert 0 <= span["start"] < span["end"] <= len(text.encode())
        text.encode()[span["start"]:span["end"]].decode("utf-8")
    holdout.append({"format":label["format"], "text":text, "spans":label["spans"]})
holdout_path = ROOT / "out" / "holdout.json"
holdout_path.write_text(json.dumps(holdout, ensure_ascii=False), encoding="utf-8")

sections = []
gate = True
for name, path, cases in [("Generated", corpus, generated), ("Holdout", holdout_path, holdout)]:
    run = subprocess.run([command, "run", "--quiet", "-p", "leaklens-core", "--example", "s4", "--", str(path)],
                         cwd=repo, capture_output=True, text=True)
    if run.returncode:
        # Cargo diagnostics do not contain fixture contents.
        raise RuntimeError(run.stderr)
    counts = json.loads(run.stdout)
    positive = sum(bool(case["spans"]) for case in cases)
    multiline = sum(bool(case["spans"]) and any(b"\n" in case["text"].encode()[s["start"]:s["end"]]
                    for s in case["spans"]) for case in cases)
    rows = [f"## {name}", "", f"{len(cases)} records: {positive} positives, {len(cases)-positive} look-alikes; {multiline} multiline positives.", "",
            "| Group | TP | FP | FN | Precision | Recall |", "|---|---:|---:|---:|---:|---:|"]
    for group, (tp, fp, fn) in sorted(counts.items()):
        precision = tp / (tp + fp) if tp + fp else 1.0
        recall = tp / (tp + fn) if tp + fn else 1.0
        # Block gates apply independently by dataset and format. Warn has a precision target.
        if group.endswith("/severity/Block") or group == "severity/Block":
            gate &= precision >= .90 and recall >= .90
        elif group.endswith("/severity/Warn") or group == "severity/Warn":
            gate &= precision >= .80
        rows.append(f"| {group} | {tp} | {fp} | {fn} | {precision:.4f} | {recall:.4f} |")
    sections.append("\n".join(rows))

report = f'''# S4 clipboard structural detection

Reproduce from repository root: `python spikes/s4-clipboard/score.py`.
Synthetic data only; no third-party Python packages. Generated seed: {SEED}.
The generator uses {sum(len(v) for v in NEGATIVES.values())} distinct look-alike templates and varied prose, punctuation,
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

{chr(10).join(sections)}

Gate: Block precision and recall >=0.90 per dataset/format/severity, and Warn
precision target >=0.80: **{'PASS' if gate else 'FAIL'}**.
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
'''
(ROOT / "RESULTS.md").write_text(report, encoding="utf-8")
print(report)
raise SystemExit(0 if gate else 1)
