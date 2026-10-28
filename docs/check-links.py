"""Check local Markdown links and GitHub-style heading anchors; no network."""
from pathlib import Path
import re
import sys
from urllib.parse import unquote, urlsplit

ROOT = Path(__file__).resolve().parents[1]
NEW_DOCS = [
    "building", "detectors", "evaluation", "limitations", "reference",
    "tester-guide", "t15-validation",
]
sources = [ROOT / "README.md", ROOT / "CONTRIBUTING.md", ROOT / "demo/README.md"]
sources += [ROOT / f"docs/{name}.md" for name in NEW_DOCS]


def anchors(path):
    seen = {}
    result = set()
    for title in re.findall(r"^#{1,6}\s+(.+?)\s*#*\s*$", path.read_text(encoding="utf-8-sig"), re.M):
        title = re.sub(r"<[^>]*>", "", title).lower()
        slug = re.sub(r"[^\w\- ]", "", title).replace(" ", "-")
        count = seen.get(slug, 0)
        seen[slug] = count + 1
        result.add(slug + (f"-{count}" if count else ""))
    return result


errors = []
checked = 0
for source in sources:
    if not source.is_file():
        errors.append(f"Missing source: {source.relative_to(ROOT)}")
        continue
    body = re.sub(r"```.*?```", "", source.read_text(encoding="utf-8-sig"), flags=re.S)
    for target in re.findall(r"!?\[[^\]]*\]\(([^)]+)\)", body):
        target = target.strip().split(' "', 1)[0].strip("<>")
        url = urlsplit(target)
        if url.scheme or url.netloc:
            continue
        checked += 1
        destination = (source.parent / unquote(url.path)).resolve() if url.path else source
        if not destination.exists():
            errors.append(f"{source.relative_to(ROOT)}: missing {target}")
        elif url.fragment and destination.suffix == ".md" and unquote(url.fragment) not in anchors(destination):
            errors.append(f"{source.relative_to(ROOT)}: missing anchor {target}")
print(f"Checked {checked} local links/anchors in {len(sources)} Markdown files; {len(errors)} errors.")
for error in errors:
    print(error)
sys.exit(bool(errors))
