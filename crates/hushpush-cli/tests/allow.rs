use assert_cmd::Command;
use std::{fs, path::Path, process::Output};
use tempfile::{Builder, TempDir};

const EMAIL: &str = "synthetic@hushpush.company";
const EMAIL_UPPER: &str = "SYNTHETIC@HUSHPUSH.COMPANY";
const CARD: &str = "4242 4242 4242 4242";
const BLOCK_EMAIL: &str = "[detectors.email]\nseverity = \"block\"\n";

fn safe(text: &str) {
    for value in [EMAIL, EMAIL_UPPER, CARD, "4242424242424242"] {
        assert!(!text.contains(value), "synthetic value disclosed");
    }
}

fn git(root: &Path, args: &[&str]) -> String {
    let output = std::process::Command::new("git")
        .current_dir(root)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env(
            "GIT_CONFIG_GLOBAL",
            if cfg!(windows) { "NUL" } else { "/dev/null" },
        )
        .args(args)
        .output()
        .unwrap();
    safe(&String::from_utf8_lossy(&output.stdout));
    safe(&String::from_utf8_lossy(&output.stderr));
    assert!(output.status.success(), "synthetic Git operation failed");
    String::from_utf8(output.stdout).unwrap()
}

fn fixture() -> TempDir {
    let dir = Builder::new()
        .prefix("synthetic-t9-")
        .tempdir_in(env!("CARGO_MANIFEST_DIR"))
        .unwrap();
    git(dir.path(), &["init", "-q", "-b", "main"]);
    fs::create_dir(dir.path().join("input")).unwrap();
    config(dir.path(), BLOCK_EMAIL);
    dir
}

fn command(root: &Path) -> Command {
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("hushpush"));
    cmd.current_dir(root).arg("-C").arg(root);
    cmd
}

fn checked(output: Output, code: i32) -> String {
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    safe(&text);
    assert_eq!(
        output.status.code(),
        Some(code),
        "unexpected status: {text}"
    );
    if code != 2 {
        assert!(output.stderr.is_empty(), "unexpected stderr: {text}");
    }
    text
}

fn run(root: &Path, args: &[&str], code: i32) -> String {
    checked(command(root).args(args).output().unwrap(), code)
}

fn scan(root: &Path, code: i32) -> String {
    run(root, &["scan", "dir", "input"], code)
}

fn config(root: &Path, content: &str) {
    safe(content);
    fs::write(root.join(".hushpush.toml"), content).unwrap();
}

fn commit(root: &Path) {
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
            "-c",
            "core.hooksPath=disabled-synthetic-hooks",
            "commit",
            "-qm",
            "synthetic fixture",
        ],
    );
}

fn fingerprint(output: &str) -> String {
    let rest = output.split("fp=").nth(1).expect("finding fingerprint");
    let fp: String = rest.chars().take(16).collect();
    assert_eq!(fp.len(), 16);
    assert!(
        fp.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    );
    fp
}

fn assert_private_files(root: &Path) {
    let dir = root.join(".git/hushpush");
    assert_eq!(fs::read(dir.join("key")).unwrap().len(), 32);
    safe(&fs::read_to_string(root.join(".hushpush.toml")).unwrap());
    safe(&fs::read_to_string(dir.join("allow.toml")).unwrap());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(dir.join("key")).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
}

#[test]
fn concurrent_first_discovery_uses_one_complete_key() {
    use hushpush_core::{Sensitive, allow::LocalStore};
    use std::sync::Barrier;

    let mut failed_calls = 0;
    let mut failed_rounds = 0;
    for _ in 0..50 {
        let dir = fixture();
        assert!(!dir.path().join(".git/hushpush").exists());
        let barrier = Barrier::new(16);
        let fingerprints = std::thread::scope(|scope| {
            let workers: Vec<_> = (0..16)
                .map(|_| {
                    let barrier = &barrier;
                    let root = dir.path();
                    scope.spawn(move || {
                        barrier.wait();
                        LocalStore::discover(root, false).map(|store| {
                            store
                                .expect("synthetic repository has local storage")
                                .fingerprint("email", &Sensitive::new(EMAIL.into()))
                        })
                    })
                })
                .collect();
            workers
                .into_iter()
                .map(|worker| worker.join().expect("discovery worker panicked"))
                .collect::<Vec<_>>()
        });
        let failures = fingerprints.iter().filter(|result| result.is_err()).count();
        failed_calls += failures;
        failed_rounds += usize::from(failures != 0);
        let successful: Vec<_> = fingerprints
            .iter()
            .filter_map(|fp| fp.as_ref().ok())
            .collect();
        let persisted = LocalStore::discover(dir.path(), false)
            .unwrap()
            .unwrap()
            .fingerprint("email", &Sensitive::new(EMAIL.into()));
        assert!(
            successful.iter().all(|fp| **fp == persisted),
            "concurrent callers used different keys"
        );
        assert_eq!(
            fs::read(dir.path().join(".git/hushpush/key"))
                .unwrap()
                .len(),
            32
        );
        assert_eq!(
            fs::read_dir(dir.path().join(".git/hushpush"))
                .unwrap()
                .count(),
            1
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            for (path, mode) in [(".git/hushpush", 0o700), (".git/hushpush/key", 0o600)] {
                assert_eq!(
                    fs::metadata(dir.path().join(path))
                        .unwrap()
                        .permissions()
                        .mode()
                        & 0o777,
                    mode
                );
            }
        }
    }
    assert_eq!(
        failed_calls, 0,
        "{failed_calls}/800 discovery calls failed in {failed_rounds}/50 fresh repositories"
    );
}

#[test]
fn invalid_existing_key_lengths_fail_closed() {
    use hushpush_core::allow::{AllowError, LocalStore};

    for length in [0, 31, 33] {
        let dir = fixture();
        let storage = dir.path().join(".git/hushpush");
        fs::create_dir(&storage).unwrap();
        fs::write(storage.join("key"), vec![0; length]).unwrap();
        assert!(matches!(
            LocalStore::discover(dir.path(), false),
            Err(AllowError::Storage)
        ));
        assert_eq!(
            fs::metadata(storage.join("key")).unwrap().len(),
            length as u64
        );
        assert_eq!(fs::read_dir(storage).unwrap().count(), 1);
    }
}

#[test]
fn scope_suppressions_are_masked_audited_and_can_be_disabled() {
    let dir = fixture();
    fs::write(dir.path().join("input/log"), EMAIL).unwrap();
    config(
        dir.path(),
        &format!(
            "{BLOCK_EMAIL}\n[[allow]]\nid = \"docs-fixtures\"\npaths = [\"**/log\"]\nreason = \"Synthetic fixture\"\n"
        ),
    );
    let output = scan(dir.path(), 0);
    assert!(output.contains("[EMAIL"));
    assert!(output.contains("suppressed by allow:docs-fixtures"));
    assert!(output.contains("1 suppressed (docs-fixtures=1)"));
    assert!(output.contains("Scanned 1 file"));
    let output = run(dir.path(), &["--no-allow", "scan", "dir", "input"], 1);
    assert!(!output.contains("suppressed by"));
    let listed = run(dir.path(), &["allow", "list"], 0);
    for expected in ["docs-fixtures", "Synthetic fixture", "active"] {
        assert!(listed.contains(expected), "missing list detail: {listed}");
    }
}

#[test]
fn detector_scope_expiry_and_unused_rules_are_reported() {
    let dir = fixture();
    fs::write(dir.path().join("input/log"), format!("{EMAIL}\n{CARD}")).unwrap();
    config(
        dir.path(),
        &format!(
            "{BLOCK_EMAIL}\n[[allow]]\nid = \"email-only\"\npaths = [\"**/log\"]\ndetectors = [\"email\"]\nreason = \"Synthetic email\"\n\
         [[allow]]\nid = \"expired-card\"\npaths = [\"**/log\"]\nreason = \"Expired synthetic scope\"\nexpires = \"2000-01-01\"\n\
         [[allow]]\nid = \"unused\"\npaths = [\"elsewhere/**\"]\nreason = \"Unmatched synthetic scope\"\n"
        ),
    );
    let output = scan(dir.path(), 1);
    let card_line = output.lines().find(|line| line.contains("[CARD")).unwrap();
    assert!(!card_line.contains("suppressed by"));
    assert!(output.contains("suppressed by allow:email-only"));
    assert!(output.contains("Expired allow rules ignored: expired-card"));
    assert!(output.contains("Unused allow rules: unused"));
    let listed = run(dir.path(), &["allow", "list"], 0);
    assert!(listed.contains("2000-01-01") && listed.contains("expired"));
}

#[test]
fn malformed_scope_rules_fail_with_the_rule_id() {
    let dir = fixture();
    for fields in [
        "paths = [\"*\"]\nreason = \"x\"",
        "paths = [\"**\"]\nreason = \"x\"",
        "paths = [\"**/*\"]\nreason = \"x\"",
        "paths = [\"**/log\"]",
        "paths = [\"**/log\"]\nreason = \"\"",
        "paths = [\"**/log\"]\nreason = \"x\"\nexpires = \"2027-02-30\"",
        "paths = [\"**/log\"]\nreason = \"x\"\nunknown = true",
        "paths = [\"**/log\"]\nreason = \"x\"\n[[allow]]\nid = \"invalid-rule\"\npaths = [\"other/**\"]\nreason = \"x\"",
    ] {
        config(
            dir.path(),
            &format!("[[allow]]\nid = \"invalid-rule\"\n{fields}\n"),
        );
        assert!(scan(dir.path(), 2).contains("invalid-rule"));
    }
}

#[test]
fn value_allow_roundtrip_is_private_and_does_not_dirty_the_repository() {
    let dir = fixture();
    fs::write(dir.path().join("input/log"), EMAIL).unwrap();
    commit(dir.path());
    let fp = fingerprint(&scan(dir.path(), 1));
    run(
        dir.path(),
        &[
            "allow",
            "value",
            &fp,
            "--reason",
            "Synthetic value",
            "--expires",
            "2099-01-31",
        ],
        0,
    );
    let output = scan(dir.path(), 0);
    assert!(output.contains(&format!("suppressed by value:{fp}")));
    assert!(output.contains("1 suppressed (value=1)"));
    let output = run(dir.path(), &["--no-allow", "scan", "dir", "input"], 1);
    assert!(!output.contains("suppressed by"));
    let listed = run(dir.path(), &["allow", "list"], 0);
    for expected in [&fp, "Synthetic value", "2099-01-31", "active"] {
        assert!(listed.contains(expected));
    }
    assert_private_files(dir.path());
    assert!(git(dir.path(), &["status", "--porcelain"]).is_empty());
    let allow = fs::read_to_string(dir.path().join(".git/hushpush/allow.toml")).unwrap();
    assert!(allow.contains("added"));
    assert!(!allow.contains("detector"));
    run(
        dir.path(),
        &["allow", "value", &fp, "--reason", "duplicate"],
        2,
    );
    for invalid in ["short", "0123456789ABCDEFa", "0123456789abcdef0"] {
        run(dir.path(), &["allow", "value", invalid, "--reason", "x"], 2);
    }
}

#[test]
fn normalization_and_repository_keys_define_fingerprints() {
    let first = fixture();
    fs::write(first.path().join("input/log"), EMAIL).unwrap();
    let first_fp = fingerprint(&scan(first.path(), 1));
    fs::write(first.path().join("input/log"), EMAIL_UPPER).unwrap();
    assert_eq!(first_fp, fingerprint(&scan(first.path(), 1)));
    let second = fixture();
    fs::write(second.path().join("input/log"), EMAIL).unwrap();
    assert_ne!(first_fp, fingerprint(&scan(second.path(), 1)));
    fs::write(first.path().join("input/log"), CARD).unwrap();
    let spaced = fingerprint(&scan(first.path(), 1));
    fs::write(first.path().join("input/log"), "4242424242424242").unwrap();
    assert_eq!(spaced, fingerprint(&scan(first.path(), 1)));
}

#[test]
fn expired_value_rules_are_ignored_and_unused_values_are_audited() {
    let dir = fixture();
    fs::write(dir.path().join("input/log"), EMAIL).unwrap();
    let fp = fingerprint(&scan(dir.path(), 1));
    run(
        dir.path(),
        &[
            "allow",
            "value",
            &fp,
            "--reason",
            "Expired synthetic value",
            "--expires",
            "2000-01-01",
        ],
        0,
    );
    let output = scan(dir.path(), 1);
    assert!(output.contains("Expired allow rules ignored:"));
    assert!(output.contains(&fp));
    assert!(!output.contains("suppressed by"));
    let unused = "0123456789abcdef";
    run(
        dir.path(),
        &[
            "allow",
            "value",
            unused,
            "--reason",
            "Unused synthetic value",
        ],
        0,
    );
    let output = scan(dir.path(), 1);
    assert!(output.contains("Unused allow rules:"));
    assert!(output.contains(unused));
}

#[test]
fn linked_worktree_shares_the_common_key_and_value_rules() {
    let dir = fixture();
    fs::write(dir.path().join("input/log"), EMAIL).unwrap();
    commit(dir.path());
    let fp = fingerprint(&scan(dir.path(), 1));
    run(
        dir.path(),
        &[
            "allow",
            "value",
            &fp,
            "--reason",
            "Shared synthetic fixture",
        ],
        0,
    );
    let linked = dir.path().join(".git/synthetic-linked-worktree");
    git(
        dir.path(),
        &[
            "worktree",
            "add",
            "--detach",
            linked.to_str().unwrap(),
            "HEAD",
        ],
    );
    let output = scan(&linked, 0);
    assert_eq!(fp, fingerprint(&output));
    assert!(output.contains(&format!("suppressed by value:{fp}")));
    assert!(!linked.join("hushpush").exists());
    assert!(git(&linked, &["status", "--porcelain"]).is_empty());
    assert_private_files(dir.path());
}

#[test]
fn forget_requires_yes_and_removes_local_state() {
    let dir = fixture();
    fs::write(dir.path().join("input/log"), EMAIL).unwrap();
    scan(dir.path(), 1);
    assert!(dir.path().join(".git/hushpush").is_dir());
    assert!(run(dir.path(), &["forget"], 2).contains("--yes"));
    assert!(dir.path().join(".git/hushpush").is_dir());
    run(dir.path(), &["forget", "--yes"], 0);
    assert!(!dir.path().join(".git/hushpush").exists());
}

#[test]
fn outgoing_configuration_change_has_an_informational_notice() {
    let dir = fixture();
    commit(dir.path());
    let base = git(dir.path(), &["rev-parse", "HEAD"]);
    let remote = dir.path().join(".git/synthetic-remote");
    git(
        dir.path(),
        &["init", "--bare", "-q", remote.to_str().unwrap()],
    );
    git(
        dir.path(),
        &["remote", "add", "synthetic", remote.to_str().unwrap()],
    );
    git(
        dir.path(),
        &["push", "-q", "synthetic", "HEAD:refs/heads/main"],
    );
    config(dir.path(), "version = 1\n");
    commit(dir.path());
    let head = git(dir.path(), &["rev-parse", "HEAD"]);
    let hook_input = format!(
        "refs/heads/main {} refs/heads/main {}\n",
        head.trim(),
        base.trim()
    );
    for output in [
        run(dir.path(), &["scan", "commits", "HEAD~1..HEAD"], 0),
        run(dir.path(), &["scan", "push", "--remote", "synthetic"], 0),
        checked(
            command(dir.path())
                .args(["hook", "pre-push", "synthetic", "synthetic-url"])
                .write_stdin(hook_input)
                .output()
                .unwrap(),
            0,
        ),
    ] {
        assert!(output.contains(
            "Notice: outgoing commits modify .hushpush.toml (allow rules may have changed)"
        ));
        assert!(output.contains("1 blob"));
        assert!(!output.contains("1 blobs"));
    }
}

#[test]
fn reasons_with_detected_values_and_malformed_local_rules_never_disclose_them() {
    let dir = fixture();
    fs::write(dir.path().join("input/log"), EMAIL).unwrap();
    let fp = fingerprint(&scan(dir.path(), 1));
    for reason in [EMAIL, CARD] {
        run(dir.path(), &["allow", "value", &fp, "--reason", reason], 2);
    }
    let local = dir.path().join(".git/hushpush/allow.toml");
    if local.exists() {
        safe(&fs::read_to_string(&local).unwrap());
    }
    for text in [
        format!("[[value]]\nfp = \"{fp}\"\nreason = \"{EMAIL}\"\nadded = \"2026-01-01\"\n"),
        format!(
            "[[value]]\nfp = \"{fp}\"\nreason = \"safe\"\nadded = \"2026-01-01\"\nraw = \"{CARD}\"\n"
        ),
        format!("invalid = \"{EMAIL}"),
    ] {
        fs::write(&local, text).unwrap();
        run(dir.path(), &["allow", "list"], 2);
        scan(dir.path(), 2);
    }
    for fields in [
        format!("paths = [\"**/log\"]\nreason = \"{EMAIL}\""),
        format!("paths = [\"input/{EMAIL}\"]\nreason = \"Synthetic fixture\""),
    ] {
        fs::write(
            dir.path().join(".hushpush.toml"),
            format!("[[allow]]\nid = \"private-rule\"\n{fields}\n"),
        )
        .unwrap();
        assert!(scan(dir.path(), 2).contains("private-rule"));
    }
}

#[test]
fn outside_git_has_no_fingerprints_and_value_allows_are_unavailable() {
    let dir = Builder::new()
        .prefix("synthetic-t9-no-git-")
        .tempdir_in(env!("CARGO_MANIFEST_DIR"))
        .unwrap();
    fs::create_dir(dir.path().join("input")).unwrap();
    fs::write(dir.path().join("input/log"), EMAIL).unwrap();
    config(dir.path(), BLOCK_EMAIL);
    let outside = || {
        let mut command = Command::new(assert_cmd::cargo::cargo_bin!("hushpush"));
        command
            .current_dir(dir.path())
            .env("GIT_CEILING_DIRECTORIES", dir.path().parent().unwrap())
            .args(["--config", ".hushpush.toml"]);
        command
    };
    let output = checked(
        outside().args(["scan", "dir", "input"]).output().unwrap(),
        1,
    );
    assert!(!output.contains("fp="));
    assert_eq!(output.matches("Git repository").count(), 1);
    checked(
        outside()
            .args([
                "allow",
                "value",
                "0123456789abcdef",
                "--reason",
                "Synthetic fixture",
            ])
            .output()
            .unwrap(),
        2,
    );
    assert!(!dir.path().join("hushpush").exists());
}

#[test]
fn identical_outgoing_blobs_keep_each_paths_scope_decision() {
    let dir = fixture();
    config(
        dir.path(),
        &format!(
            "{BLOCK_EMAIL}\n[[allow]]\nid = \"docs-only\"\npaths = [\"docs/**\"]\nreason = \"Synthetic documentation\"\n"
        ),
    );
    commit(dir.path());
    fs::create_dir_all(dir.path().join("docs/allowed")).unwrap();
    fs::create_dir(dir.path().join("src")).unwrap();
    fs::write(dir.path().join("docs/allowed/log"), EMAIL).unwrap();
    fs::write(dir.path().join("src/log"), EMAIL).unwrap();
    commit(dir.path());
    let output = run(dir.path(), &["scan", "commits", "HEAD~1..HEAD"], 1);
    let docs = output
        .lines()
        .find(|line| line.contains("docs/allowed/log") && line.contains("[EMAIL"))
        .expect("documentation finding");
    let source = output
        .lines()
        .find(|line| line.contains("src/log") && line.contains("[EMAIL"))
        .expect("source finding");
    assert!(docs.contains("suppressed by allow:docs-only"));
    assert!(!source.contains("suppressed by"));
    assert_eq!(fingerprint(docs), fingerprint(source));
    assert!(output.contains("1 suppressed (docs-only=1)"));
}

#[test]
fn outgoing_config_notice_survives_reuse_of_an_existing_blob() {
    let dir = fixture();
    fs::write(dir.path().join("alias.toml"), "version = 1\n").unwrap();
    commit(dir.path());
    config(dir.path(), "version = 1\n");
    commit(dir.path());
    let output = run(dir.path(), &["scan", "commits", "HEAD~1..HEAD"], 0);
    assert!(
        output.contains(
            "Notice: outgoing commits modify .hushpush.toml (allow rules may have changed)"
        )
    );
}

#[test]
fn detected_values_in_display_paths_are_masked() {
    let dir = fixture();
    fs::write(dir.path().join("input").join(EMAIL), CARD).unwrap();
    let output = scan(dir.path(), 1);
    assert!(output.contains("[CARD"));
    assert!(output.contains("fp="));
}

#[test]
fn suppression_is_audited_during_redaction_and_content_is_still_redacted() {
    let dir = fixture();
    fs::write(dir.path().join("input/log"), EMAIL).unwrap();
    config(
        dir.path(),
        &format!(
            "{BLOCK_EMAIL}\n[[allow]]\nid = \"synthetic-log\"\npaths = [\"**/log\"]\nreason = \"Synthetic log fixture\"\n"
        ),
    );
    let output = run(dir.path(), &["redact", "input", "--out", "redacted"], 0);
    assert!(output.contains("suppressed by allow:synthetic-log"));
    assert!(output.contains("1 suppressed (synthetic-log=1)"));
    let redacted = fs::read_to_string(dir.path().join("redacted/log")).unwrap();
    safe(&redacted);
    assert_eq!(redacted, "[REDACTED:EMAIL]");
    assert!(
        fs::read(dir.path().join("input/log")).unwrap() == EMAIL.as_bytes(),
        "original content changed"
    );
}
