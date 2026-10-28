# Agent rules for this repository

hushpush is a local Rust CLI that finds likely personal data in logs and screenshots before they are pushed. The plan is in `docs/plan.md`; task IDs (T1, T2, …) and spike IDs (S1–S5) refer to it.

## Hard rules
- Use synthetic data only. Never scan, copy, or read the user's real files outside this repository.
- Never print, log, or write a detected sensitive value. Wrap matched content in `Sensitive<T>`; previews are shape-only (`[EMAIL len=21]`).
- Do not commit, push, rewrite history, or install Git hooks. The coordinator reviews and commits.
- Stay inside the paths your task brief assigns. Do not edit files outside them.
- Do not invent measurements. Every number in a results file must come from a run you did.

## Conventions
- Rust stable, edition 2024. `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, and `cargo test` must pass.
- Errors: `thiserror` in libraries, `anyhow` only in the binary.
- Comments explain why, not what. Keep them sparse.
- Licenses of new dependencies must be MIT, Apache-2.0, BSD, ISC, Zlib, or Unicode; `cargo deny check` enforces this.
- The license allowlist applies to shipped dependencies; the fuzz/ workspace may also use NCSA (libFuzzer).
- One crate-scoped exception: `webpki-root-certs` (CDLA-Permissive-2.0), a build-time dependency of ort's binary downloader. Add no other exceptions without owner approval.
- On Windows, the toolchain lives in `%USERPROFILE%\.cargo\bin`.

## Finish every task with
A short report: what changed (paths), commands run with pass/fail, acceptance criteria met or not, and open questions.
