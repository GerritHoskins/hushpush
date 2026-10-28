"""Run on Windows after dist produces its local ZIP; synthetic data only."""
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import time
import zipfile

repo = Path(__file__).resolve().parents[1]
archive = Path(sys.argv[1]).resolve()
with tempfile.TemporaryDirectory(prefix="synthetic-install-") as directory:
    root = Path(directory)
    start = time.perf_counter()
    with zipfile.ZipFile(archive) as package:
        for member in package.infolist():
            if not (root / member.filename).resolve().is_relative_to(root):
                raise ValueError("unsafe archive member")
        package.extractall(root)
    extracted = time.perf_counter()
    binary, = root.rglob("hushpush.exe")
    # Use the existing synthetic fixture, like the Linux read-only fixture mount.
    report = root / "result.json"
    env = {"SystemRoot": os.environ["SystemRoot"],
           "PATH": str(Path(os.environ["SystemRoot"]) / "System32"),
           "TEMP": str(root), "TMP": str(root)}
    result = subprocess.run([str(binary), "scan", "dir", str(repo / "demo/evidence"),
        "--include-reserved", "--no-user-patterns", "--json", str(report)],
        cwd=root, env=env, capture_output=True, timeout=60)
    finished = time.perf_counter()
    data = json.loads(report.read_text(encoding="utf-8"))
    bbox = any(f["category"] == "email" and
               Path(f["source"]["path"]).name == "screenshot.png" and
               f["location"]["kind"] == "bbox" and
               f["location"]["w"] > 0 and f["location"]["h"] > 0
               for f in data["findings"])
    timings = dict(extraction_seconds=extracted-start, scan_seconds=finished-extracted,
                   total_seconds=finished-start, exit_code=result.returncode,
                   screenshot_email_bbox=bbox)
    print(json.dumps(timings, indent=2))
    assert result.returncode == 1 and bbox and timings["total_seconds"] <= 60
