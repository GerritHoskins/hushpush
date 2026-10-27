use std::{fs, path::Path};

use assert_cmd::Command;
use tempfile::{Builder, TempDir};

const EMAIL: &str = "jane.doe@corp.example.com";
const CARD: &str = "4242 4242 4242 4242";

fn fixture() -> TempDir {
    Builder::new()
        .prefix("synthetic-t3-")
        .tempdir_in(env!("CARGO_MANIFEST_DIR"))
        .unwrap()
}

fn scan(path: &Path, include_reserved: bool, expected: i32) -> String {
    let mut command = Command::new(assert_cmd::cargo::cargo_bin!("hushpush"));
    command.args(["scan", "dir"]).arg(path);
    if include_reserved {
        command.arg("--include-reserved-domains");
    }
    let output = command.output().unwrap();
    for bytes in [&output.stdout, &output.stderr] {
        let text = String::from_utf8_lossy(bytes);
        for value in [
            EMAIL,
            CARD,
            "4242424242424242",
            "5555-5555-5555-4444",
            "378282246310005",
        ] {
            assert!(!text.contains(value), "planted value disclosed");
        }
    }
    assert_eq!(output.status.code(), Some(expected));
    assert!(output.stderr.is_empty() || expected == 2);
    if output.stdout.is_empty() {
        return String::new();
    }
    String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(|line| {
            if let Some((before, _)) = line.split_once(" fp=") {
                before
            } else {
                line
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
        + "\n"
}

#[test]
fn review_log_reports_exact_lines_columns_and_lengths() {
    let dir = fixture();
    fs::write(
        dir.path().join("review.log"),
        concat!(
            "L1 2026-10-27 4242424242424242\n",
            "L2 qty 3 4242 4242 4242 4242\n",
            "L3 card 4242 4242 4242 4242 exp 12 28\n",
            "L4 email=jane.doe@corp.example.com\n",
            "L5 4242 4242 4242 4242\n",
        ),
    )
    .unwrap();
    assert_eq!(
        scan(dir.path(), true, 1),
        concat!(
            "review.log:1:15  Block  PaymentCard  High  [CARD len=16]\n",
            "review.log:2:10  Block  PaymentCard  High  [CARD len=19]\n",
            "review.log:3:9  Block  PaymentCard  High  [CARD len=19]\n",
            "review.log:4:10  Warn  Email  High  [EMAIL len=25]\n",
            "review.log:5:4  Block  PaymentCard  High  [CARD len=19]\n",
            "Scanned 1 file\n",
            "Detectors: card@2 (block), email@3 (warn), phone@1 (warn), home_path@1 (warn), pem@1 (block), clipboard@2 (block), clipboard_field@1 (warn)\n",
            "Config: defaults\n",
            "Skipped 0\n",
            "4 blocking findings, 1 warning finding\n",
        )
    );
}

#[test]
fn mixed_directory_has_masked_findings_and_coverage_snapshot() {
    let dir = fixture();
    fs::create_dir(dir.path().join("nested")).unwrap();
    fs::write(
        dir.path().join("nested/log.txt"),
        format!("\u{feff}start\r\n😀 é {EMAIL}\r\ncard: {CARD}\n"),
    )
    .unwrap();
    fs::write(dir.path().join("image.log"), b"\x89PNG\r\n\x1a\n").unwrap();
    fs::write(dir.path().join("binary.txt"), b"\0ignored").unwrap();
    fs::write(dir.path().join("partial.log"), b"\xffplain").unwrap();
    fs::create_dir(dir.path().join(".git")).unwrap();
    fs::write(dir.path().join(".git/ignored"), CARD).unwrap();
    let output = scan(dir.path(), true, 1);
    assert_eq!(
        output,
        concat!(
            "nested/log.txt:2:5  Warn  Email  High  [EMAIL len=25]\n",
            "nested/log.txt:3:7  Block  PaymentCard  High  [CARD len=19]\n",
            "Scanned 2 files\n",
            "Detectors: card@2 (block), email@3 (warn), phone@1 (warn), home_path@1 (warn), pem@1 (block), clipboard@2 (block), clipboard_field@1 (warn)\n",
            "Config: defaults\n",
            "Skipped 3: image-decode-failed=1 (incomplete, block), binary=1 (unsupported, warn), partial-decode=1 (incomplete, block)\n",
            "1 blocking finding, 1 warning finding\n",
            "Coverage incomplete (policy: block): image-decode-failed=1 partial-decode=1\n",
            "WARNING: Coverage unsupported (policy: warn): binary=1\n"
        )
    );
}

#[test]
fn supported_text_exits_zero_with_or_without_warnings() {
    let dir = fixture();
    fs::write(dir.path().join("log"), EMAIL).unwrap();
    assert!(scan(dir.path(), false, 0).contains("No findings in supported categories."));
    assert!(scan(dir.path(), true, 0).contains("0 blocking findings, 1 warning finding"));
}

#[test]
fn each_classification_has_the_expected_exit_code() {
    for (bytes, expected, reason) in [
        (b"\x89PNG\r\n\x1a\n".as_slice(), 3, "image-decode-failed"),
        (b"\xff\xd8\xff".as_slice(), 3, "image-decode-failed"),
        (b"\0plain".as_slice(), 0, "binary"),
        (b"\xffplain".as_slice(), 3, "partial-decode"),
    ] {
        let dir = fixture();
        fs::write(dir.path().join("fixture"), bytes).unwrap();
        let output = scan(dir.path(), false, expected);
        assert!(output.contains(reason));
        assert_eq!(
            output.contains("Coverage incomplete (policy: block):"),
            expected == 3
        );
    }
}

#[test]
fn partial_decode_is_scanned_and_block_takes_precedence() {
    let dir = fixture();
    fs::write(
        dir.path().join("log"),
        [b"\xff ".as_slice(), EMAIL.as_bytes()].concat(),
    )
    .unwrap();
    let output = scan(dir.path(), true, 3);
    assert!(output.contains("log:1:3  Warn  Email  High  [EMAIL len=25]"));
    fs::write(dir.path().join("card"), CARD).unwrap();
    assert!(scan(dir.path(), true, 1).contains("Coverage incomplete (policy: block):"));
}

#[test]
fn oversized_files_are_incomplete() {
    let dir = fixture();
    use std::io::Write;
    let mut file = fs::File::create(dir.path().join("large.log")).unwrap();
    file.write_all(&[b'x'; 8192]).unwrap();
    file.set_len(32 * 1024 * 1024 + 1).unwrap();
    assert!(scan(dir.path(), false, 3).contains("too-large=1"));
}

#[test]
fn missing_or_non_directory_root_exits_two_without_echoing_path() {
    let dir = fixture();
    let missing = dir.path().join("synthetic-missing-path");
    assert!(scan(&missing, false, 2).is_empty());
    fs::write(&missing, CARD).unwrap();
    assert!(scan(&missing, false, 2).is_empty());
}

#[test]
fn synthetic_flag_is_documented() {
    let output = Command::new(assert_cmd::cargo::cargo_bin!("hushpush"))
        .args(["scan", "dir", "--help"])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(
        String::from_utf8(output.stdout)
            .unwrap()
            .contains("for synthetic test data")
    );
}

#[cfg(unix)]
#[test]
fn symlinks_are_not_followed_and_paths_cannot_inject_terminal_controls() {
    let dir = fixture();
    fs::create_dir(dir.path().join(".git")).unwrap();
    fs::write(dir.path().join(".git/log"), CARD).unwrap();
    std::os::unix::fs::symlink(dir.path().join(".git"), dir.path().join("link")).unwrap();
    fs::write(dir.path().join("fake\n\u{1b}[31m.log"), EMAIL).unwrap();
    let output = scan(dir.path(), true, 0);
    assert!(output.contains("fake??.log:1:1"));
    assert!(output.contains("symlink=1"));
    assert!(!output.contains('\u{1b}'));
}

#[test]
fn git_metadata_files_and_directories_are_skipped_but_worktrees_are_scanned() {
    let dir = fixture();
    fs::create_dir_all(dir.path().join("nested/.git")).unwrap();
    fs::create_dir_all(dir.path().join("submodule")).unwrap();
    fs::create_dir_all(dir.path().join("worktree")).unwrap();
    fs::write(dir.path().join("nested/.git/private"), CARD).unwrap();
    fs::write(dir.path().join("submodule/.git"), format!("gitdir: {CARD}")).unwrap();
    fs::write(dir.path().join("worktree/.git"), format!("gitdir: {CARD}")).unwrap();
    fs::write(dir.path().join("submodule/log.txt"), EMAIL).unwrap();
    fs::write(dir.path().join("worktree/log.txt"), EMAIL).unwrap();
    let report = hushpush_core::source::scan_dir(dir.path(), true).unwrap();
    assert_eq!(report.coverage.scanned, 2);
    assert_eq!(report.findings.len(), 2);
    assert!(
        report
            .findings
            .iter()
            .any(|f| f.path == "submodule/log.txt")
    );
    assert!(report.findings.iter().any(|f| f.path == "worktree/log.txt"));
}

#[test]
fn bidi_filename_is_safe_in_terminal_and_json() {
    let dir = fixture();
    fs::write(dir.path().join("report\u{202e}gnp.txt"), EMAIL).unwrap();
    let text = scan(dir.path(), true, 0);
    assert!(!text.contains('\u{202e}'));
    assert!(text.contains("report?gnp.txt"));
    let report = hushpush_core::source::scan_dir(dir.path(), true).unwrap();
    let mut bytes = Vec::new();
    report.write_json(&mut bytes, "dir", false).unwrap();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(json["findings"][0]["source"]["path"], "report?gnp.txt");
}

#[test]
fn directory_loop_and_external_symlink_are_listed_without_following() {
    let dir = fixture();
    let external = fixture();
    fs::write(external.path().join("unread.txt"), CARD).unwrap();
    #[cfg(unix)]
    let link = |target: &Path, path: &Path| std::os::unix::fs::symlink(target, path);
    #[cfg(windows)]
    let link = |target: &Path, path: &Path| std::os::windows::fs::symlink_dir(target, path);
    link(dir.path(), &dir.path().join("loop")).unwrap();
    link(external.path(), &dir.path().join("external")).unwrap();
    let report = hushpush_core::source::scan_dir(dir.path(), true).unwrap();
    assert_eq!(report.coverage.scanned, 0);
    assert!(report.findings.is_empty());
    assert_eq!(report.coverage.skipped.len(), 2);
    for path in ["loop", "external"] {
        assert!(
            report
                .coverage
                .skipped
                .iter()
                .any(|s| s.path == path && s.reason == hushpush_core::report::SkipReason::Symlink)
        );
    }
}

#[test]
fn unsupported_formats_have_specific_reasons_and_coverage_policies() {
    let dir = fixture();
    for (name, bytes) in [
        ("document.dat", b"%PDF-1.7".as_slice()),
        ("animation.dat", b"GIF89a"),
        ("archive.dat", b"PK\x03\x04"),
        ("movie.dat", b"\0\0\0\x18ftypisom"),
    ] {
        fs::write(dir.path().join(name), bytes).unwrap();
    }
    let report = hushpush_core::source::scan_dir(dir.path(), false).unwrap();
    assert_eq!(report.coverage.scanned, 0);
    assert_eq!(report.coverage.skipped.len(), 4);
    assert!(
        report
            .coverage
            .skipped
            .iter()
            .all(|s| s.reason.unsupported())
    );
    let output = scan(dir.path(), false, 0);
    for reason in [
        "pdf=1",
        "image-format-unsupported=1",
        "archive=1",
        "video=1",
    ] {
        assert!(output.contains(reason));
    }
    let mut config = hushpush_core::config::Config::default();
    config.policy.on_unsupported = hushpush_core::config::PolicyAction::Block;
    let report = hushpush_core::source::scan_dir_with_config(dir.path(), &config).unwrap();
    assert_eq!(
        report.exit_code(),
        hushpush_core::ExitCode::IncompleteCoverage
    );
}
