# LeakLens

> Working name. The project will be renamed before its first release, because "LeakLens" is already used by other projects.

**A local, offline pre-push check that finds personal data in screenshots and logs before they reach a Git remote.**

Developers commit more and more debugging evidence: screenshots, terminal output, device logs, and AI-agent transcripts. These artifacts can quietly contain emails, card numbers, or other personal details. Secret scanners such as [gitleaks](https://github.com/gitleaks/gitleaks), [Betterleaks](https://github.com/betterleaks/betterleaks) and [TruffleHog](https://github.com/trufflesecurity/trufflehog) look for credentials, but they don't read the text inside images. LeakLens is built to run **alongside** them, not to replace them.

> **Status: pre-alpha.** Text and screenshot scanning and the Git modes work. Redacted copies (`redact`) are in progress. Don't rely on it yet.

## What it does

- **Scans what you are actually about to publish**, not only the working tree:
  - `scan dir <PATH>`: files on disk
  - `scan staged`: the bytes in Git's index, including the temporary index Git uses for `git commit -- <path>` and `git commit -a`
  - `scan commits A..B` and `hook pre-push`: every blob new in the outgoing commits, **including intermediate commits** whose content a later commit removed
- **Reads screenshots locally** using PP-OCRv5 (mobile) models via ONNX Runtime. Findings are reported as bounding boxes.
- **Never prints what it found.** Output is shape-only, such as `[EMAIL len=21]`. In the code, matched values are wrapped in a type that cannot be formatted.
- **Fails closed.** If something couldn't be scanned (an unsupported or oversized file, OCR unavailable, a missing object in a partial clone), the scan reports **incomplete coverage** with exit code 3. It never presents that as a clean result.
- **Works offline.** No account, no upload, no LLM. The only network access is an explicit, hash-verified model download.

## Detectors (current)

| Detector | Severity | Notes |
|---|---|---|
| Payment card | block | 13–19 digits, known IIN prefixes, Luhn check, tolerant of adjacent dates and IDs |
| Email | warn | reserved test domains (`example.com`, `.test`, …) are ignored unless `--include-reserved-domains` is passed |

Phone numbers, home-directory paths, PEM private keys, clipboard-history records and user-defined patterns are planned.

## Quick start (from source)

```bash
cargo build --release
cargo xtask fetch-models          # PP-OCRv5 mobile + ONNX Runtime, SHA-256 verified, into ./models
./target/release/leaklens scan dir ./evidence
./target/release/leaklens hook print > .git/hooks/pre-push   # review it, then chmod +x
```

Exit codes: `0` no blocking findings · `1` blocking findings · `2` usage/config error · `3` incomplete coverage · `4` internal error.

## Measured OCR quality (synthetic corpus)

These numbers come from 60 generated screenshots (terminal, chat, devtools and log-viewer layouts at 1x, 1.5x and 2x; fake data only):

| | 1x | 2x |
|---|---|---|
| Email recall | 93% | 100% |
| Card recall | 92% | 100% |
| False findings on look-alikes | 0 | 0 |

The worst warm time was 1.54 s per 1080p image on an 8-core desktop. Laptop performance is not measured yet. See [`xtask/RESULTS.md`](xtask/RESULTS.md) and the spikes in [`spikes/`](spikes/).

## Limitations

A run with no findings does **not** mean an artifact is safe to publish. LeakLens misses:
- unsupported formats (PDF, video)
- non-Latin scripts
- rotated, stylized or low-resolution text
- free-form personal prose (names, addresses, messages)
- values split across lines or images
- encoded content
- secrets other than those listed above, so run a secret scanner as well

## Docs

- [Implementation plan](docs/plan.md)
- [Agent and contributor rules](AGENTS.md)

## License

Dual-licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your option. The OCR models are Apache-2.0 (PaddlePaddle), and ONNX Runtime is MIT.
