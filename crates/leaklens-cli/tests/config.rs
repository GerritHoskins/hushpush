use assert_cmd::Command;
use std::{fs, path::Path, process::Output};
use tempfile::{Builder, TempDir};

const EMAIL: &str = "synthetic@leaklens.company";
const RESERVED: &str = "demo@example.com";
const CARD: &str = "4242 4242 4242 4242";

fn fixture() -> TempDir {
    let dir = Builder::new()
        .prefix("synthetic-t8-")
        .tempdir_in(env!("CARGO_MANIFEST_DIR"))
        .unwrap();
    git(dir.path(), &["init", "-q"]);
    fs::create_dir(dir.path().join("input")).unwrap();
    dir
}
fn git(root: &Path, args: &[&str]) {
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
    assert!(
        output.status.success(),
        "synthetic repository operation failed"
    );
}
fn command(root: &Path) -> Command {
    let mut command = Command::new(assert_cmd::cargo::cargo_bin!("leaklens"));
    command.current_dir(root).arg("-C").arg(root);
    command
}
fn checked(output: Output, expected: i32) -> String {
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    for value in [EMAIL, RESERVED, CARD, "4242424242424242"] {
        assert!(!text.contains(value), "synthetic value disclosed");
    }
    assert_eq!(
        output.status.code(),
        Some(expected),
        "unexpected status: {text}"
    );
    if expected != 2 {
        assert!(output.stderr.is_empty(), "unexpected stderr");
    }
    text
}
fn scan(root: &Path, extra: &[&str], expected: i32) -> String {
    checked(
        command(root)
            .args(extra)
            .args(["scan", "dir", "input"])
            .output()
            .unwrap(),
        expected,
    )
}
fn config(root: &Path, text: &str) {
    fs::write(root.join(".leaklens.toml"), text).unwrap();
}

#[test]
fn invalid_config_keys_versions_values_and_hard_caps_exit_two_without_source_text() {
    let dir = fixture();
    fs::write(dir.path().join("input/log"), EMAIL).unwrap();
    for (text, key) in [
        ("mystery = true", "mystery"),
        (
            "\"policy.on_incomplete\" = \"warn\"",
            "policy.on_incomplete",
        ),
        (
            "[\"detectors.email\"]\nseverity = \"block\"",
            "detectors.email",
        ),
        ("[policy]\nunknown = true", "policy.unknown"),
        ("version = 2", "version"),
        (
            "[policy]\non_incomplete = \"invalid\"",
            "policy.on_incomplete",
        ),
        (
            "[detectors.email]\nseverity = \"invalid\"",
            "detectors.email.severity",
        ),
        ("[limits]\nmax_text_mib = 257", "limits.max_text_mib"),
        ("[limits]\nmax_image_mib = 201", "limits.max_image_mib"),
        (
            "[limits]\nmax_image_megapixels = 201",
            "limits.max_image_megapixels",
        ),
        (
            "[limits]\nocr_timeout_secs = 121",
            "limits.ocr_timeout_secs",
        ),
    ] {
        config(dir.path(), text);
        let output = scan(dir.path(), &[], 2);
        assert!(
            output.contains(key),
            "configuration error must identify key: {output}"
        );
        assert_eq!(output.lines().count(), 1);
    }
}

#[test]
fn missing_explicit_config_is_a_config_error() {
    let dir = fixture();
    let output = scan(dir.path(), &["--config", "synthetic-missing.toml"], 2);
    assert!(output.contains("config"));
    assert_eq!(output.lines().count(), 1);
}

#[test]
fn explicit_config_precedes_git_root_config_and_identifies_checksum() {
    let dir = fixture();
    fs::write(dir.path().join("input/log"), EMAIL).unwrap();
    config(dir.path(), "[detectors.email]\nseverity = \"off\"");
    fs::write(
        dir.path().join("alternate.toml"),
        "[detectors.email]\nseverity = \"block\"",
    )
    .unwrap();
    let output = scan(dir.path(), &["--config", "alternate.toml"], 1);
    assert!(output.contains("Config: alternate.toml (sha256 "));
    assert!(output.contains("email@3 (block)"));
    assert!(output.contains("1 blocking, 0 warning findings"));
}

#[test]
fn email_severity_block_off_and_default_footer() {
    let dir = fixture();
    fs::write(dir.path().join("input/log"), EMAIL).unwrap();
    let output = scan(dir.path(), &[], 0);
    assert!(output.contains("Config: defaults"));
    assert!(output.contains("Scanned 1 file"));
    assert!(output.contains("Detectors: card@2 (block), email@3 (warn)"));
    assert_eq!(output.matches("Scanned ").count(), 1);
    assert!(!output.contains("Scanned:"));
    assert!(!output.contains("Skipped:"));
    config(dir.path(), "[detectors.email]\nseverity = \"block\"");
    assert!(scan(dir.path(), &[], 1).contains("email@3 (block)"));
    config(dir.path(), "[detectors.email]\nseverity = \"off\"");
    let output = scan(dir.path(), &[], 0);
    assert!(!output.contains("email@"));
    assert!(!output.contains("[EMAIL"));
    assert!(output.contains("No findings in supported categories."));
}

#[test]
fn include_reserved_domains_cli_overrides_config() {
    let dir = fixture();
    fs::write(dir.path().join("input/log"), RESERVED).unwrap();
    config(
        dir.path(),
        "[detectors.email]\nseverity = \"block\"\ninclude_reserved_domains = false",
    );
    assert!(scan(dir.path(), &[], 0).contains("No findings in supported categories."));
    assert!(scan(dir.path(), &["--include-reserved-domains"], 1).contains("[EMAIL len=16]"));
}

#[test]
fn binary_is_unsupported_warn_by_default_and_block_when_configured() {
    let dir = fixture();
    fs::write(dir.path().join("input/binary"), b"\0synthetic").unwrap();
    let output = scan(dir.path(), &[], 0);
    assert!(output.contains("binary=1 (unsupported, warn)"));
    config(dir.path(), "[policy]\non_unsupported = \"block\"");
    let output = scan(dir.path(), &[], 3);
    assert!(output.contains("binary=1 (unsupported, block)"));
}

#[test]
fn lfs_pointer_is_skipped_in_directory_and_staged_modes() {
    let dir = fixture();
    fs::write(
        dir.path().join("input/pointer"),
        concat!(
            "version https://git-lfs.github.com/spec/v1\n",
            "oid sha256:0000000000000000000000000000000000000000000000000000000000000000\n",
            "size 123\n",
        ),
    )
    .unwrap();
    for mode in ["dir", "staged"] {
        git(dir.path(), &["add", "input/pointer"]);
        let mut cmd = command(dir.path());
        cmd.args(["scan", mode]);
        if mode == "dir" {
            cmd.arg("input");
        }
        let output = checked(cmd.output().unwrap(), 0);
        assert!(output.contains("lfs-pointer=1 (unsupported, warn)"));
        assert!(output.contains("real content"));
    }
    config(dir.path(), "[policy]\non_unsupported = \"block\"");
    assert!(
        checked(
            command(dir.path())
                .args(["scan", "staged"])
                .output()
                .unwrap(),
            3
        )
        .contains("lfs-pointer")
    );
}

#[test]
fn staged_scan_reads_working_tree_config_and_discovers_git_root_from_subdirectory() {
    let dir = fixture();
    config(dir.path(), "[detectors.email]\nseverity = \"off\"");
    fs::write(dir.path().join("input/log"), EMAIL).unwrap();
    git(dir.path(), &["add", ".leaklens.toml", "input/log"]);
    config(dir.path(), "version = 1\n");
    let subdir = dir.path().join("input");
    let output = checked(
        command(&subdir).args(["scan", "staged"]).output().unwrap(),
        0,
    );
    assert!(output.contains("[EMAIL"));
    assert!(output.contains("email@3 (warn)"));
    assert!(output.contains(".leaklens.toml (sha256 "));
    config(dir.path(), "[detectors.email]\nseverity = \"block\"");
    let output = checked(
        command(&subdir).args(["scan", "staged"]).output().unwrap(),
        1,
    );
    assert!(output.contains("email@3 (block)"));
    // The actual process CWD also selects the working tree when -C is absent.
    let output = checked(
        Command::new(assert_cmd::cargo::cargo_bin!("leaklens"))
            .current_dir(&subdir)
            .args(["scan", "dir", "."])
            .output()
            .unwrap(),
        1,
    );
    assert!(output.contains("[EMAIL"));
}

#[test]
fn malformed_image_and_partial_text_obey_warn_and_cli_block_override() {
    let dir = fixture();
    for (bytes, reason) in [
        (b"\x89PNG\r\n\x1a\n".as_slice(), "image-decode-failed"),
        (b"\xffsynthetic text".as_slice(), "partial-decode"),
    ] {
        fs::write(dir.path().join("input/fixture"), bytes).unwrap();
        config(dir.path(), "version = 1");
        let output = scan(dir.path(), &[], 3);
        assert!(output.contains(reason));
        assert!(output.contains("Coverage incomplete (policy: block)"));
        config(dir.path(), "[policy]\non_incomplete = \"warn\"");
        let output = scan(dir.path(), &[], 0);
        assert!(output.contains(reason));
        assert!(output.contains("Coverage incomplete (policy: warn)"));
        assert!(scan(dir.path(), &["--on-incomplete", "block"], 3).contains("policy: block"));
    }
}

#[test]
fn missing_ocr_models_obey_warn_and_cli_block_override() {
    let dir = fixture();
    fs::write(
        dir.path().join("input/screenshot.png"),
        include_bytes!("fixtures/demo.png"),
    )
    .unwrap();
    let missing = dir.path().join("synthetic-missing-models");
    let run = |extra: &[&str], code| {
        checked(
            command(dir.path())
                .arg("--models")
                .arg(&missing)
                .args(extra)
                .args(["scan", "dir", "input"])
                .output()
                .unwrap(),
            code,
        )
    };
    assert!(run(&[], 3).contains("ocr-unavailable"));
    config(dir.path(), "[policy]\non_incomplete = \"warn\"");
    let output = run(&[], 0);
    assert!(output.contains("Coverage incomplete (policy: warn)"));
    assert!(output.contains("OCR unavailable:"));
    assert!(run(&["--on-incomplete", "block"], 3).contains("policy: block"));
}

#[test]
fn configured_text_size_limit_is_used_before_decode() {
    let dir = fixture();
    fs::write(
        dir.path().join("input/large.log"),
        vec![b'x'; 1024 * 1024 + 1],
    )
    .unwrap();
    config(dir.path(), "[limits]\nmax_text_mib = 1");
    assert!(scan(dir.path(), &[], 3).contains("too-large"));
    config(dir.path(), "[limits]\nmax_text_mib = 2");
    assert!(scan(dir.path(), &[], 0).contains("Scanned 1 file"));
}

#[test]
fn oversized_unsupported_content_keeps_its_policy_in_directory_staged_and_commits() {
    for (prefix, reason) in [
        (b"\0".as_slice(), "binary"),
        (
            b"version https://git-lfs.github.com/spec/v1\n".as_slice(),
            "lfs-pointer",
        ),
    ] {
        let dir = fixture();
        git(
            dir.path(),
            &[
                "-c",
                "user.name=Synthetic",
                "-c",
                "user.email=synthetic@example.com",
                "-c",
                "commit.gpgsign=false",
                "-c",
                "core.hooksPath=disabled-synthetic-hooks",
                "commit",
                "--allow-empty",
                "-m",
                "base",
            ],
        );
        let mut bytes = vec![b'x'; 2 * 1024 * 1024];
        bytes[..prefix.len()].copy_from_slice(prefix);
        fs::write(dir.path().join("input/large"), bytes).unwrap();
        git(dir.path(), &["add", "input/large"]);
        let policy = "[policy]\non_incomplete = \"warn\"\non_unsupported = \"block\"\n[limits]\nmax_text_mib = 1\nmax_image_mib = 1\n";
        config(dir.path(), policy);
        assert!(scan(dir.path(), &[], 3).contains(reason));
        let output = command(dir.path())
            .args(["scan", "staged"])
            .output()
            .unwrap();
        assert!(checked(output, 3).contains(reason));
        git(
            dir.path(),
            &[
                "-c",
                "user.name=Synthetic",
                "-c",
                "user.email=synthetic@example.com",
                "-c",
                "commit.gpgsign=false",
                "-c",
                "core.hooksPath=disabled-synthetic-hooks",
                "commit",
                "-m",
                "unsupported",
            ],
        );
        let output = command(dir.path())
            .args(["scan", "commits", "HEAD~1..HEAD"])
            .output()
            .unwrap();
        let text = checked(output, 3);
        assert!(text.contains(reason));
        assert!(text.contains("blobs in HEAD~1..HEAD"));
        assert!(!text.contains("ref updates") && !text.contains("outgoing"));
        config(
            dir.path(),
            &policy.replace("on_unsupported = \"block\"", "on_unsupported = \"warn\""),
        );
        assert!(scan(dir.path(), &[], 0).contains(reason));
    }
}
