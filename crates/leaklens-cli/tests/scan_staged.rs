use std::{
    fs,
    io::Write,
    path::Path,
    process::{Command, Output, Stdio},
};
use tempfile::{Builder, TempDir};

const CARD: &str = "4242 4242 4242 4242";
const EMAIL: &str = "synthetic@leaklens.company";

fn fixture() -> TempDir {
    Builder::new()
        .prefix("t6-")
        .tempdir_in(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .unwrap()
                .parent()
                .unwrap(),
        )
        .unwrap()
}

fn git(root: &Path, args: &[&str]) -> Output {
    let output = Command::new("git")
        .current_dir(root)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env(
            "GIT_CONFIG_GLOBAL",
            if cfg!(windows) { "NUL" } else { "/dev/null" },
        )
        .args(args)
        .output()
        .unwrap();
    assert!(output.status.success(), "fixture git command failed");
    output
}

fn repo() -> TempDir {
    let dir = fixture();
    git(dir.path(), &["init", "-q"]);
    dir
}

fn commit(root: &Path) {
    git(
        root,
        &[
            "-c",
            "user.name=Synthetic",
            "-c",
            "user.email=synthetic@example.com",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "-qm",
            "synthetic",
        ],
    );
}

fn assert_output(output: Output, expected: i32) -> String {
    for bytes in [&output.stdout, &output.stderr] {
        let text = String::from_utf8_lossy(bytes);
        for value in [CARD, EMAIL, "4242424242424242"] {
            assert!(!text.contains(value), "planted value disclosed");
        }
    }
    assert_eq!(output.status.code(), Some(expected));
    if expected != 2 && expected != 4 {
        assert!(output.stderr.is_empty());
    }
    String::from_utf8(output.stdout).unwrap()
}

fn scan(root: &Path, expected: i32) -> String {
    assert_output(
        Command::new(assert_cmd::cargo::cargo_bin!("leaklens"))
            .arg("-C")
            .arg(root)
            .args(["scan", "staged"])
            .output()
            .unwrap(),
        expected,
    )
}

fn stage(root: &Path, path: &str, bytes: impl AsRef<[u8]>) {
    fs::write(root.join(path), bytes).unwrap();
    git(root, &["add", "--", path]);
}

fn index_blob(root: &Path, mode: &str, path: &str, bytes: &[u8]) -> String {
    let mut child = Command::new("git")
        .current_dir(root)
        .args(["hash-object", "-w", "--stdin"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(bytes).unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    let oid = String::from_utf8(output.stdout).unwrap().trim().to_owned();
    git(
        root,
        &[
            "update-index",
            "--add",
            "--cacheinfo",
            &format!("{mode},{oid},{path}"),
        ],
    );
    oid
}

#[test]
fn unborn_head_scans_staged_card_while_directory_is_clean() {
    let dir = repo();
    stage(dir.path(), "log", CARD);
    fs::write(dir.path().join("log"), "ordinary text").unwrap();
    let output = scan(dir.path(), 1);
    assert!(output.contains("[CARD len=19]"));
    assert!(output.contains("Scanned 1 staged blob"));
    let report = leaklens_core::source::git::scan_staged(dir.path()).unwrap();
    let oid = String::from_utf8(git(dir.path(), &["rev-parse", ":log"]).stdout).unwrap();
    assert_eq!(report.findings[0].blob_oid.as_deref(), Some(oid.trim()));
    let output = assert_output(
        Command::new(assert_cmd::cargo::cargo_bin!("leaklens"))
            .args(["scan", "dir"])
            .arg(dir.path())
            .output()
            .unwrap(),
        0,
    );
    assert!(output.contains("No findings in supported categories."));
    assert!(
        leaklens_core::source::scan_dir(dir.path(), false)
            .unwrap()
            .findings
            .is_empty()
    );
}

#[test]
fn working_tree_card_is_not_scanned() {
    let dir = repo();
    stage(dir.path(), "log", "ordinary text");
    fs::write(dir.path().join("log"), CARD).unwrap();
    assert!(scan(dir.path(), 0).contains("No findings in supported categories."));
}

#[test]
fn modified_and_deleted_entries_after_commit() {
    let dir = repo();
    stage(dir.path(), "modified", "ordinary text");
    stage(dir.path(), "deleted", CARD);
    commit(dir.path());
    git(dir.path(), &["rm", "-q", "deleted"]);
    assert!(scan(dir.path(), 0).contains("Scanned 0 staged blobs"));
    stage(dir.path(), "modified", CARD);
    fs::write(dir.path().join("modified"), "ordinary text").unwrap();
    assert!(scan(dir.path(), 1).contains("modified:1:1"));
}

#[test]
fn symlink_blob_is_text_without_resolving_target_and_type_changes_are_scanned() {
    let dir = repo();
    // The target exists and contains a card; following the link would block the scan.
    fs::write(dir.path().join(EMAIL), CARD).unwrap();
    stage(dir.path(), "link", "ordinary text");
    commit(dir.path());
    let oid = index_blob(dir.path(), "120000", "link", EMAIL.as_bytes());
    let output = scan(dir.path(), 0);
    assert!(output.contains(&format!("[EMAIL len={}]", EMAIL.len())));
    assert!(output.contains("Scanned 1 staged blob"));
    let report = leaklens_core::source::git::scan_staged(dir.path()).unwrap();
    assert_eq!(report.findings[0].blob_oid.as_deref(), Some(oid.as_str()));
}

#[test]
fn submodule_is_listed_without_incomplete_coverage() {
    let dir = repo();
    stage(dir.path(), "log", "ordinary text");
    commit(dir.path());
    let oid = String::from_utf8(git(dir.path(), &["rev-parse", "HEAD"]).stdout).unwrap();
    git(
        dir.path(),
        &[
            "update-index",
            "--add",
            "--cacheinfo",
            &format!("160000,{},module", oid.trim()),
        ],
    );
    let output = scan(dir.path(), 0);
    assert!(output.contains("Scanned 0 staged blobs"));
    assert!(output.contains("submodule=1"));
    assert!(!output.contains("Coverage incomplete"));
}

#[test]
fn staged_png_is_incomplete() {
    let dir = repo();
    stage(dir.path(), "image", b"\x89PNG\r\n\x1a\n");
    assert!(scan(dir.path(), 3).contains("image-decode-failed=1"));
}

#[test]
fn oversized_blob_is_skipped_before_content_scan() {
    let dir = repo();
    let mut bytes = vec![b'x'; 32 * 1024 * 1024 + 1];
    bytes[..CARD.len()].copy_from_slice(CARD.as_bytes());
    index_blob(dir.path(), "100644", "large", &bytes);
    let output = scan(dir.path(), 3);
    assert!(output.contains("too-large=1"));
    assert!(output.contains("Scanned 0 staged blobs"));
}

#[test]
fn not_a_repository_and_missing_git_are_one_line_usage_errors() {
    let dir = fixture();
    // Stop discovery at this synthetic boundary instead of finding the host checkout.
    fs::write(
        dir.path().join(".git"),
        "gitdir: nonexistent-synthetic-git-dir\n",
    )
    .unwrap();
    for missing_git in [false, true] {
        let mut command = Command::new(assert_cmd::cargo::cargo_bin!("leaklens"));
        command.arg("-C").arg(dir.path()).args(["scan", "staged"]);
        if missing_git {
            command.env("PATH", "");
        }
        let output = command.output().unwrap();
        let stderr = String::from_utf8(output.stderr.clone()).unwrap();
        assert!(assert_output(output, 2).is_empty());
        assert_eq!(stderr.lines().count(), 1);
        assert_eq!(
            stderr.trim(),
            if missing_git {
                "leaklens: git is unavailable"
            } else {
                "leaklens: not a git repository"
            }
        );
    }
}

#[test]
fn other_git_failures_are_generic_internal_errors() {
    let dir = repo();
    fs::write(dir.path().join(".git/index"), "synthetic invalid index").unwrap();
    let output = Command::new(assert_cmd::cargo::cargo_bin!("leaklens"))
        .arg("-C")
        .arg(dir.path())
        .args(["scan", "staged"])
        .output()
        .unwrap();
    let stderr = output.stderr.clone();
    assert!(assert_output(output, 4).is_empty());
    assert_eq!(
        String::from_utf8(stderr).unwrap().trim(),
        "leaklens: git operation failed"
    );
}

#[test]
fn nested_directory_and_user_config_do_not_change_index_selection() {
    let dir = repo();
    stage(dir.path(), "log", CARD);
    fs::create_dir(dir.path().join("nested")).unwrap();
    git(dir.path(), &["config", "diff.relative", "true"]);
    let output = assert_output(
        Command::new(assert_cmd::cargo::cargo_bin!("leaklens"))
            .current_dir(dir.path().join("nested"))
            .args(["scan", "staged"])
            .env("GIT_CONFIG_COUNT", "1")
            .env("GIT_CONFIG_KEY_0", "diff.ignoreSubmodules")
            .env("GIT_CONFIG_VALUE_0", "all")
            .output()
            .unwrap(),
        1,
    );
    assert!(output.contains("log:1:1"));
}

#[cfg(unix)]
#[test]
fn non_utf8_path_is_displayed_lossily() {
    use std::{ffi::OsStr, os::unix::ffi::OsStrExt};
    let dir = repo();
    fs::write(dir.path().join(OsStr::from_bytes(b"synthetic-\xff")), CARD).unwrap();
    git(dir.path(), &["add", "--all"]);
    assert!(scan(dir.path(), 1).contains("synthetic-\u{fffd}:1:1"));
}
#[test]
fn multiple_blobs_keep_protocol_boundaries_and_directory_findings_have_no_oid() {
    let dir = repo();
    stage(dir.path(), "a file", "");
    stage(dir.path(), "b file", EMAIL);
    stage(dir.path(), "c file", CARD);
    let output = assert_output(
        Command::new(assert_cmd::cargo::cargo_bin!("leaklens"))
            .args(["scan", "staged", "-C"])
            .arg(dir.path())
            .output()
            .unwrap(),
        1,
    );
    assert!(output.contains("Scanned 3 staged blobs"));
    assert!(output.contains("b file:1:1"));
    assert!(output.contains("c file:1:1"));
    let report = leaklens_core::source::scan_dir(dir.path(), false).unwrap();
    assert_eq!(report.findings.len(), 2);
    assert!(
        report
            .findings
            .iter()
            .all(|finding| finding.blob_oid.is_none())
    );
}

#[cfg(any(unix, windows))]
#[test]
fn native_staged_symlink_is_not_followed_when_os_permits() {
    let dir = repo();
    fs::write(dir.path().join(EMAIL), CARD).unwrap();
    #[cfg(unix)]
    let result = std::os::unix::fs::symlink(EMAIL, dir.path().join("link"));
    #[cfg(windows)]
    let result = std::os::windows::fs::symlink_file(EMAIL, dir.path().join("link"));
    if let Err(error) = result {
        if matches!(
            error.kind(),
            std::io::ErrorKind::PermissionDenied | std::io::ErrorKind::Unsupported
        ) || error.raw_os_error() == Some(1314)
        {
            eprintln!("native symlink check skipped: OS permission unavailable");
            return;
        }
        panic!("synthetic symlink creation failed");
    }
    git(dir.path(), &["-c", "core.symlinks=true", "add", "link"]);
    let output = scan(dir.path(), 0);
    assert!(output.contains(&format!("[EMAIL len={}]", EMAIL.len())));
    assert!(output.contains("Scanned 1 staged blob"));
}

fn install_hook(root: &Path, injected_environment: &str) {
    install_hook_with_args(root, injected_environment, "");
}

fn install_hook_with_args(root: &Path, injected_environment: &str, args: &str) {
    let binary = assert_cmd::cargo::cargo_bin!("leaklens")
        .to_str()
        .unwrap()
        .replace('\\', "/")
        .replace('\'', "'\\''");
    let hook = root.join(".git/hooks/pre-commit");
    fs::write(
        &hook,
        format!("#!/bin/sh\n{injected_environment}exec '{binary}' {args} scan staged\n"),
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(hook, fs::Permissions::from_mode(0o755)).unwrap();
    }
}

fn hook_blocks(root: &Path, args: &[&str]) {
    let before = git(root, &["rev-parse", "HEAD"]).stdout;
    let output = Command::new("git")
        .current_dir(root)
        .args([
            "-c",
            "user.name=Synthetic",
            "-c",
            "user.email=synthetic@example.com",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "-qm",
            "synthetic",
        ])
        .args(args)
        .output()
        .unwrap();
    assert!(!output.status.success(), "hook failed to block commit");
    for bytes in [&output.stdout, &output.stderr] {
        let text = String::from_utf8_lossy(bytes);
        assert!(!text.contains(CARD), "planted value disclosed");
        assert!(
            !text.contains("4242424242424242"),
            "planted value disclosed"
        );
    }
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        text.contains("[CARD len=19]"),
        "hook did not detect synthetic card"
    );
    assert!(text.contains("Scanned 1 staged blob"));
    assert_eq!(git(root, &["rev-parse", "HEAD"]).stdout, before);
    let head = git(root, &["show", "HEAD:log.txt"]).stdout;
    assert!(
        !String::from_utf8_lossy(&head).contains(CARD),
        "card reached HEAD"
    );
}

fn clean_committed_repo() -> TempDir {
    let dir = repo();
    stage(dir.path(), "log.txt", "ordinary text\n");
    commit(dir.path());
    dir
}

#[test]
fn partial_commit_hook_reads_temporary_index() {
    let dir = clean_committed_repo();
    install_hook(dir.path(), "");
    fs::write(dir.path().join("log.txt"), CARD).unwrap();
    hook_blocks(dir.path(), &["--", "log.txt"]);
}

#[test]
fn commit_all_hook_reads_updated_index() {
    let dir = clean_committed_repo();
    install_hook(dir.path(), "");
    fs::write(dir.path().join("log.txt"), CARD).unwrap();
    hook_blocks(dir.path(), &["-a"]);
}

#[test]
fn staged_commit_hook_blocks() {
    let dir = clean_committed_repo();
    install_hook(dir.path(), "");
    stage(dir.path(), "log.txt", CARD);
    fs::write(dir.path().join("log.txt"), "ordinary text\n").unwrap();
    hook_blocks(dir.path(), &[]);
}

#[test]
fn linked_worktree_hook_reads_its_index() {
    let dir = clean_committed_repo();
    install_hook(dir.path(), "");
    let worktree = fixture();
    let path = worktree.path().to_str().unwrap();
    git(
        dir.path(),
        &["worktree", "add", "-q", "-b", "synthetic-worktree", path],
    );
    stage(worktree.path(), "log.txt", CARD);
    hook_blocks(worktree.path(), &[]);
}

#[test]
fn explicit_directory_ignores_inherited_routing() {
    let dir = clean_committed_repo();
    stage(dir.path(), "log.txt", CARD);
    let output = assert_output(
        Command::new(assert_cmd::cargo::cargo_bin!("leaklens"))
            .arg("-C")
            .arg(dir.path())
            .args(["scan", "staged"])
            .env("GIT_INDEX_FILE", "nonexistent-synthetic-index")
            .env("GIT_DIR", "nonexistent-synthetic-git-dir")
            .env("GIT_WORK_TREE", "nonexistent-synthetic-work-tree")
            .output()
            .unwrap(),
        1,
    );
    assert!(output.contains("[CARD len=19]"));
}

#[test]
fn explicit_directory_hook_ignores_bogus_index() {
    let dir = clean_committed_repo();
    let path = dir
        .path()
        .to_str()
        .unwrap()
        .replace('\\', "/")
        .replace('\'', "'\\''");
    install_hook_with_args(
        dir.path(),
        "export GIT_INDEX_FILE='nonexistent-synthetic-index'\n",
        &format!("-C '{path}'"),
    );
    stage(dir.path(), "log.txt", CARD);
    hook_blocks(dir.path(), &[]);
}

#[test]
fn hook_strips_injected_config_and_external_diff() {
    let dir = clean_committed_repo();
    // Malformed config would make Git fail before producing any scan output.
    install_hook(
        dir.path(),
        "export GIT_CONFIG_PARAMETERS='invalid-synthetic-config'\nexport GIT_EXTERNAL_DIFF='nonexistent-synthetic-helper'\n",
    );
    stage(dir.path(), "log.txt", CARD);
    hook_blocks(dir.path(), &[]);
}

#[test]
fn relative_repository_and_index_routing_are_preserved() {
    let dir = clean_committed_repo();
    stage(dir.path(), "log.txt", CARD);
    fs::rename(
        dir.path().join(".git/index"),
        dir.path().join(".git/synthetic-index"),
    )
    .unwrap();
    let output = assert_output(
        Command::new(assert_cmd::cargo::cargo_bin!("leaklens"))
            .current_dir(dir.path())
            .args(["scan", "staged"])
            .env("GIT_DIR", ".git")
            .env("GIT_INDEX_FILE", ".git/synthetic-index")
            .output()
            .unwrap(),
        1,
    );
    assert!(output.contains("[CARD len=19]"));
}
