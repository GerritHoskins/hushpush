use std::{
    fs,
    path::Path,
    process::{Command, Output},
};

fn fixture() -> tempfile::TempDir {
    let dir = tempfile::Builder::new()
        .prefix("t12-json-")
        .tempdir_in(env!("CARGO_MANIFEST_DIR"))
        .unwrap();
    fs::create_dir(dir.path().join("input")).unwrap();
    let result = Command::new("git")
        .current_dir(dir.path())
        .args(["init", "-q"])
        .output()
        .unwrap();
    assert!(result.status.success());
    dir
}

fn run(root: &Path, target: &str) -> Output {
    Command::new(assert_cmd::cargo::cargo_bin!("hushpush"))
        .arg("-C")
        .arg(root)
        .args(["scan", "dir", "input", "--json", target])
        .output()
        .unwrap()
}

fn validate(bytes: &[u8]) -> serde_json::Value {
    let value = serde_json::from_slice(bytes).unwrap();
    let schema: serde_json::Value =
        serde_json::from_str(include_str!("../../../docs/report.schema.json")).unwrap();
    assert!(
        jsonschema::validator_for(&schema).unwrap().is_valid(&value),
        "JSON schema mismatch"
    );
    value
}

#[test]
fn json_stdout_routes_human_report_and_preserves_scan_exit_codes() {
    let dir = fixture();
    for (bytes, code) in [
        (b"ordinary".as_slice(), 0),
        (b"4242 4242 4242 4242".as_slice(), 1),
        (b"lossy\xff".as_slice(), 3),
    ] {
        fs::write(dir.path().join("input/log"), bytes).unwrap();
        let output = run(dir.path(), "-");
        assert_eq!(output.status.code(), Some(code));
        assert_eq!(validate(&output.stdout)["exit_code"], code);
        assert!(String::from_utf8_lossy(&output.stderr).contains("Scanned 1 file"));
        assert!(!String::from_utf8_lossy(&output.stderr).contains("0 suppressed"));
    }
}

#[test]
fn report_replaces_target_and_warns_only_for_unignored_files() {
    let dir = fixture();
    fs::write(dir.path().join("report.json"), "old report").unwrap();
    fs::hard_link(dir.path().join("report.json"), dir.path().join("old-link")).unwrap();
    let output = run(dir.path(), "report.json");
    assert_eq!(output.status.code(), Some(0));
    validate(&fs::read(dir.path().join("report.json")).unwrap());
    assert_eq!(
        fs::read(dir.path().join("old-link")).unwrap(),
        b"old report"
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("Scanned 0 files"));
    assert_eq!(
        String::from_utf8_lossy(&output.stderr),
        "report file is not git-ignored; it lists paths and fingerprints\n"
    );
    fs::write(dir.path().join(".gitignore"), "report.json\n").unwrap();
    let output = run(dir.path(), "report.json");
    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());
    validate(&fs::read(dir.path().join("report.json")).unwrap());
    assert!(!fs::read_dir(dir.path()).unwrap().any(|e| {
        e.unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".hushpush-report-")
    }));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(dir.path().join("report.json"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
    }
}

#[test]
fn rejected_targets_and_write_errors_still_print_human_report() {
    let dir = fixture();
    fs::create_dir(dir.path().join("synthetic-target-dir")).unwrap();
    for (target, code) in [("synthetic-target-dir", 2), ("missing/report.json", 4)] {
        let output = run(dir.path(), target);
        assert_eq!(output.status.code(), Some(code));
        assert!(String::from_utf8_lossy(&output.stdout).contains("Scanned 0 files"));
        assert!(!String::from_utf8_lossy(&output.stderr).contains(target));
        assert!(!dir.path().join("missing").exists());
    }
}

#[test]
fn symlink_target_is_refused_without_touching_referent() {
    let dir = fixture();
    let original = dir.path().join("original");
    let link = dir.path().join("link");
    fs::write(&original, "original bytes").unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(&original, &link).unwrap();
    #[cfg(windows)]
    {
        if let Err(error) = std::os::windows::fs::symlink_file(&original, &link) {
            assert_eq!(
                error.kind(),
                std::io::ErrorKind::PermissionDenied,
                "unexpected symlink fixture failure"
            );
            return;
        }
    }
    let output = run(dir.path(), "link");
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(fs::read(original).unwrap(), b"original bytes");
    assert!(fs::symlink_metadata(link).unwrap().is_symlink());
}

#[test]
fn config_report_contains_full_file_hash_without_config_path() {
    let dir = fixture();
    fs::write(dir.path().join(".hushpush.toml"), "").unwrap();
    let output = run(dir.path(), "-");
    assert_eq!(output.status.code(), Some(0));
    let report = validate(&output.stdout);
    assert_eq!(report["config"]["source"], "file");
    assert_eq!(
        report["config"]["sha256"],
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
}
