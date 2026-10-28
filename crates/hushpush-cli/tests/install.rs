use assert_cmd::Command;
use std::fs;
#[test]
fn directory_scan_fails_closed_when_git_is_missing_inside_a_repository() {
    let directory = tempfile::Builder::new()
        .prefix("synthetic-install-")
        .tempdir_in(env!("CARGO_MANIFEST_DIR"))
        .unwrap();
    // Keep a repository marker even when the tests run from a source archive.
    fs::create_dir(directory.path().join(".git")).unwrap();
    fs::write(
        directory.path().join("log.txt"),
        b"synthetic build completed",
    )
    .unwrap();
    let output = Command::new(assert_cmd::cargo::cargo_bin!("hushpush"))
        .current_dir(directory.path())
        .env("PATH", "")
        .args(["scan", "dir", ".", "--no-user-patterns"])
        .output()
        .unwrap();
    assert_eq!(
        output.status.code(),
        Some(2),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stderr).contains("git is unavailable"));
}

#[test]
fn standalone_directory_scan_does_not_require_git() {
    let directory = tempfile::Builder::new()
        .prefix("synthetic-standalone-install-")
        .tempdir()
        .unwrap();
    fs::write(
        directory.path().join("log.txt"),
        b"synthetic build completed",
    )
    .unwrap();
    let output = Command::new(assert_cmd::cargo::cargo_bin!("hushpush"))
        .current_dir(directory.path())
        .env("PATH", "")
        .args(["scan", "dir", ".", "--no-user-patterns"])
        .output()
        .unwrap();
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
}
