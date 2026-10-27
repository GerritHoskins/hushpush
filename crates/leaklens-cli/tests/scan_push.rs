use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    time::Instant,
};
use tempfile::{Builder, TempDir};
const CARD: &str = "4242 4242 4242 4242";
const ZERO: &str = "0000000000000000000000000000000000000000";

fn safe(output: &Output) {
    for bytes in [&output.stdout, &output.stderr] {
        let text = String::from_utf8_lossy(bytes);
        for value in [CARD, "4242424242424242"] {
            assert!(!text.contains(value), "planted value disclosed");
        }
    }
}
fn git(root: &Path, args: &[&str]) -> Output {
    let output = Command::new("git")
        .current_dir(root)
        .args(args)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env(
            "GIT_CONFIG_GLOBAL",
            if cfg!(windows) { "NUL" } else { "/dev/null" },
        )
        .output()
        .unwrap();
    safe(&output);
    assert!(output.status.success(), "fixture git command failed");
    output
}
fn oid(root: &Path, revision: &str) -> String {
    String::from_utf8(git(root, &["rev-parse", revision]).stdout)
        .unwrap()
        .trim()
        .into()
}
fn commit(root: &Path, name: &str, text: &str) -> String {
    fs::write(root.join(name), text).unwrap();
    git(root, &["add", "--all"]);
    git(
        root,
        &[
            "-c",
            "user.name=Synthetic",
            "-c",
            "user.email=s@example.com",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "-qm",
            "synthetic",
        ],
    );
    oid(root, "HEAD")
}
struct Fixture {
    _temp: TempDir,
    root: PathBuf,
    bare: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let temp = Builder::new()
            .prefix("t7-")
            .tempdir_in(
                Path::new(env!("CARGO_MANIFEST_DIR"))
                    .parent()
                    .unwrap()
                    .parent()
                    .unwrap(),
            )
            .unwrap();
        let root = temp.path().join("w");
        let bare = temp.path().join("r");
        fs::create_dir(&root).unwrap();
        git(
            temp.path(),
            &["init", "--bare", "-q", bare.to_str().unwrap()],
        );
        git(&root, &["init", "-q", "-b", "main"]);
        git(&root, &["remote", "add", "origin", bare.to_str().unwrap()]);
        commit(&root, "base", "ordinary base");
        git(&root, &["push", "-qu", "origin", "main"]);
        Self {
            _temp: temp,
            root,
            bare,
        }
    }
    fn line(&self, local: &str, remote: &str) -> String {
        format!("refs/heads/main {local} refs/heads/main {remote}\n")
    }
}
fn run(root: &Path, args: &[&str], input: &str, code: i32) -> String {
    let mut child = Command::new(assert_cmd::cargo::cargo_bin!("leaklens"))
        .arg("-C")
        .arg(root)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    safe(&output);
    assert_eq!(output.status.code(), Some(code), "unexpected exit code");
    if code != 2 && code != 4 {
        assert!(output.stderr.is_empty());
    }
    String::from_utf8(output.stdout).unwrap()
}
fn hook(f: &Fixture, remote: &str, input: &str, code: i32) -> String {
    run(
        &f.root,
        &["hook", "pre-push", remote, "synthetic-url"],
        input,
        code,
    )
}
#[test]
fn intermediate_blob_and_commits_range() {
    let f = Fixture::new();
    let base = oid(&f.root, "HEAD");
    let c1 = commit(&f.root, "log", CARD);
    let blob = oid(&f.root, &format!("{c1}:log"));
    let c2 = commit(&f.root, "log", "ordinary replacement");
    let input = f.line(&c2, &base);
    let text = hook(&f, "origin", &input, 1);
    assert!(text.contains("[CARD len=19]"));
    assert!(text.contains("Scanned 2 outgoing blobs across 1 ref updates"));
    let report =
        leaklens_core::source::git::scan_pre_push(&f.root, false, "origin", &input).unwrap();
    assert_eq!(
        report.report.findings[0].blob_oid.as_deref(),
        Some(blob.as_str())
    );
    let text = run(
        &f.root,
        &["scan", "commits", &format!("{base}..{c2}")],
        "",
        1,
    );
    assert!(text.contains("[CARD len=19]"));
}
#[test]
fn new_branch_excludes_tracking_blobs_and_union_dedupes() {
    let f = Fixture::new();
    let head = commit(&f.root, "new", "ordinary new");
    let line = f.line(&head, ZERO);
    let text = hook(&f, "origin", &line, 0);
    assert!(text.contains("Scanned 1 outgoing blobs across 1 ref updates"));
    assert!(!text.contains("approximate"));
    let second = format!("refs/heads/other {head} refs/heads/other {ZERO}\n");
    let text = hook(&f, "origin", &(line + &second), 0);
    assert!(text.contains("Scanned 1 outgoing blobs across 2 ref updates"));
}
#[test]
fn deletion_empty_and_malformed_input() {
    let f = Fixture::new();
    let text = hook(&f, "origin", &f.line(ZERO, &oid(&f.root, "HEAD")), 0);
    assert!(text.contains("Scanned 0 outgoing blobs across 0 ref updates"));
    assert!(hook(&f, "origin", "", 0).contains("Nothing to push"));
    for input in ["bad\n", "\n", "a not-an-oid b not-an-oid\n"] {
        hook(&f, "origin", input, 2);
    }
    let zero64 = "0".repeat(64);
    hook(&f, "origin", &f.line(&zero64, &zero64), 0);
}
#[test]
fn unknown_remote_is_approximate_and_still_detects() {
    let f = Fixture::new();
    let head = commit(&f.root, "log", CARD);
    let text = hook(&f, "origin", &f.line(&head, &"a".repeat(40)), 1);
    assert!(text.contains("Coverage approximate (over-scan)"));
    assert!(text.contains("[CARD len=19]"));
    let head = commit(&f.root, "log", "ordinary");
    // A separate benign branch proves approximation itself does not fail closed.
    git(&f.root, &["checkout", "-q", "-b", "benign", "origin/main"]);
    let benign = commit(&f.root, "other", "benign");
    let text = hook(&f, "origin", &f.line(&benign, &"b".repeat(40)), 0);
    assert!(text.contains("Coverage approximate (over-scan)"));
    assert_ne!(head, benign);
}
#[test]
fn url_remote_scans_full_history() {
    let f = Fixture::new();
    let head = commit(&f.root, "log", CARD);
    let text = hook(&f, f.bare.to_str().unwrap(), &f.line(&head, ZERO), 1);
    assert!(text.contains("Scanned 2 outgoing blobs"));
    assert!(text.contains("full history can be slow"));
    assert!(!text.contains("approximate"));
}
#[test]
fn scan_push_fallback_override_and_detached_head() {
    let f = Fixture::new();
    commit(&f.root, "log", CARD);
    assert!(run(&f.root, &["scan", "push"], "", 1).contains("Scanned 1 outgoing blobs"));
    git(&f.root, &["config", "--unset", "branch.main.remote"]);
    git(&f.root, &["config", "--unset", "branch.main.merge"]);
    assert!(run(&f.root, &["scan", "push"], "", 1).contains("Scanned 1 outgoing blobs"));
    assert!(
        run(&f.root, &["scan", "push", "--remote", "elsewhere"], "", 1).contains("full history")
    );
    git(&f.root, &["checkout", "-q", "--detach"]);
    run(&f.root, &["scan", "push"], "", 2);
}
#[test]
fn invalid_ranges_are_usage_errors() {
    let f = Fixture::new();
    for range in [
        "HEAD...HEAD",
        "..HEAD",
        "HEAD..",
        "bad..HEAD",
        "HEAD..bad",
        "HEAD",
        "HEAD..HEAD..HEAD",
    ] {
        run(&f.root, &["scan", "commits", range], "", 2);
    }
    assert!(
        run(&f.root, &["scan", "commits", "HEAD..HEAD"], "", 0)
            .contains("Scanned 0 outgoing blobs")
    );
}
#[test]
fn partial_clone_never_fetches_missing_blobs() {
    let f = Fixture::new();
    let source = f._temp.path().join("s");
    let clone = f._temp.path().join("p");
    git(
        f._temp.path(),
        &[
            "clone",
            "-q",
            f.bare.to_str().unwrap(),
            source.to_str().unwrap(),
        ],
    );
    // Bare HEAD may still name master; explicitly select the synthetic main branch.
    git(&source, &["checkout", "-q", "main"]);
    commit(&source, "log", CARD);
    git(&source, &["config", "uploadpack.allowFilter", "true"]);
    let url = format!(
        "file://{}{}",
        if cfg!(windows) { "/" } else { "" },
        source.to_str().unwrap().replace('\\', "/")
    );
    git(
        f._temp.path(),
        &[
            "clone",
            "-q",
            "--filter=blob:none",
            "--no-checkout",
            &url,
            clone.to_str().unwrap(),
        ],
    );
    // The source is an explicitly owned child of this test fixture.
    fs::remove_dir_all(&source).unwrap();
    let trace = f._temp.path().join("trace");
    let head = oid(&clone, "HEAD");
    let mut child = Command::new(assert_cmd::cargo::cargo_bin!("leaklens"))
        .arg("-C")
        .arg(&clone)
        .args(["hook", "pre-push", "url-remote", "synthetic-url"])
        .env("GIT_TRACE", &trace)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(f.line(&head, ZERO).as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    safe(&output);
    assert_eq!(output.status.code(), Some(3));
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("missing-object="));
    assert!(text.contains("Coverage incomplete"));
    // command() deliberately strips inherited tracing; no fetch or trace output is permitted.
    assert!(!trace.exists());
    assert!(output.stderr.is_empty());
}
#[test]
fn printed_hook_blocks_real_push_and_preserves_remote() {
    let f = Fixture::new();
    let before = oid(&f.bare, "refs/heads/main");
    let root = f._temp.path().join("c");
    git(
        f._temp.path(),
        &[
            "clone",
            "-q",
            "-b",
            "main",
            f.bare.to_str().unwrap(),
            root.to_str().unwrap(),
        ],
    );
    let script = run(&root, &["hook", "print"], "", 0);
    assert!(script.starts_with("#!/bin/sh\n"));
    assert!(script.contains("exec leaklens hook pre-push \"$@\""));
    assert!(script.contains("--no-verify"));
    let hook = root.join(".git/hooks/pre-push");
    assert!(!hook.exists());
    fs::write(&hook, script).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&hook, fs::Permissions::from_mode(0o755)).unwrap();
    }
    commit(&root, "log", CARD);
    let mut paths = vec![
        assert_cmd::cargo::cargo_bin!("leaklens")
            .parent()
            .unwrap()
            .to_path_buf(),
    ];
    paths.extend(std::env::split_paths(&std::env::var_os("PATH").unwrap()));
    let output = Command::new("git")
        .current_dir(&root)
        .args(["push", "origin", "main"])
        .env("PATH", std::env::join_paths(paths).unwrap())
        .output()
        .unwrap();
    safe(&output);
    assert!(!output.status.success());
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(text.contains("[CARD len=19]"));
    assert_eq!(oid(&f.bare, "refs/heads/main"), before);
}
#[test]
fn hundred_commit_timing() {
    let f = Fixture::new();
    let base = oid(&f.root, "HEAD");
    for n in 0..100 {
        commit(&f.root, "log", &format!("ordinary text {n}"));
    }
    let head = oid(&f.root, "HEAD");
    let start = Instant::now();
    let text = hook(&f, "origin", &f.line(&head, &base), 0);
    println!(
        "hook pre-push, 100 small text commits: {:?}",
        start.elapsed()
    );
    assert!(text.contains("Scanned 100 outgoing blobs across 1 ref updates"));
}
#[cfg(unix)]
#[test]
fn nul_protocol_preserves_newline_paths() {
    let f = Fixture::new();
    let head = commit(&f.root, "line\nfile", CARD);
    let report =
        leaklens_core::source::git::scan_pre_push(&f.root, false, "origin", &f.line(&head, ZERO))
            .unwrap();
    assert_eq!(report.report.findings[0].path, "line\nfile");
    let text = hook(&f, "origin", &f.line(&head, ZERO), 1);
    assert!(text.contains("line?file:1:1"));
}
