# Changelog

## 0.1.0 - 2026-10-29

First release of hushpush, a local, offline privacy check for logs and
screenshots before a Git push. It complements a secret scanner; it does not
replace one.

### Highlights

- `scan dir`, `scan staged`, `scan push`, `scan commits A..B`, a native
  pre-push hook (`hushpush hook print`), and pre-commit hooks `hushpush` and
  `hushpush-pre-push`.
- Detectors: payment cards, email, phone, home-path usernames, private PEM
  blocks, clipboard records and fields, and user-defined patterns.
- PNG/JPEG screenshots through embedded PP-OCRv5 mobile OCR with bounding boxes.
  ONNX Runtime is linked statically; scanning downloads nothing.
- `redact` writes reviewed copies (text replacement, opaque image masks,
  metadata removed) and rescans them. Originals are untouched and nothing is
  staged for you.
- Findings are shape-only (`[EMAIL len=16]`); no detected value is printed.
  JSON reports follow `hushpush.report/v1`. Value allows are repository-local
  HMAC fingerprints.
- Install with the shell or PowerShell installer, Homebrew
  (`GerritHoskins/tap/hushpush`), npm (`@gerrithoskins/hushpush`), or the
  archives for Linux x86_64/arm64, macOS x86_64/arm64 and Windows x86_64.

### Known limitations

- Linux binaries need glibc 2.35 or newer. Windows needs the Visual C++
  2015–2022 x64 Redistributable.
- The Windows binary is unsigned, so SmartScreen may warn on first run.
- OCR is Latin-only. On the synthetic corpus, OCR email recall at 2× scale is
  0.9167 against a 0.95 target (`corpus/RESULTS.md`).
- macOS and arm64 binaries are built and packaged in CI but have not been
  run-tested by the maintainer.
- Not published on crates.io; `cargo install hushpush-cli` and
  `cargo binstall` do not work.
- See [docs/limitations.md](https://github.com/GerritHoskins/hushpush/blob/v0.1.0/docs/limitations.md)
  for detector and format limits.
