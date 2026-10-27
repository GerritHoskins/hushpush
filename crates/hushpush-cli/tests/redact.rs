use assert_cmd::Command;
use std::{fs, path::Path, process::Output};
use tempfile::{Builder, TempDir};
fn dir() -> TempDir {
    Builder::new()
        .prefix("synthetic-t5-")
        .tempdir_in(env!("CARGO_MANIFEST_DIR"))
        .unwrap()
}
fn run(source: &Path, out: &Path) -> Output {
    Command::new(assert_cmd::cargo::cargo_bin!("hushpush"))
        .arg("redact")
        .arg(source)
        .arg("--out")
        .arg(out)
        .arg("--include-reserved-domains")
        .output()
        .unwrap()
}
fn masked(output: &Output) -> String {
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    for value in [
        "demo@example.com",
        "4242 4242 4242 4242",
        "4242424242424242",
    ] {
        assert!(!text.contains(value), "synthetic value disclosed");
    }
    text
}
fn sha256(path: &Path) -> Vec<u8> {
    let mut command;
    if cfg!(windows) {
        command = std::process::Command::new("powershell");
        command
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                "(Get-FileHash -Algorithm SHA256 -LiteralPath $env:HUSHPUSH_TEST_FILE).Hash",
            ])
            .env("HUSHPUSH_TEST_FILE", path);
    } else if cfg!(target_os = "macos") {
        command = std::process::Command::new("shasum");
        command.args(["-a", "256"]).arg(path);
    } else {
        command = std::process::Command::new("sha256sum");
        command.arg(path);
    }
    let output = command.output().unwrap();
    assert!(output.status.success());
    output.stdout
}
#[test]
fn text_copies_preserve_inputs_and_unchanged_files_have_no_copy() {
    let dir = dir();
    let source = dir.path().join("input");
    fs::create_dir_all(source.join("nested")).unwrap();
    let input = source.join("nested/log.txt");
    let original = b"\xef\xbb\xbfheader\r\ndemo@example.com 4242 4242 4242 4242\r\n";
    fs::write(&input, original).unwrap();
    fs::write(source.join("empty.log"), b"ordinary content\n").unwrap();
    let before = sha256(&input);
    let out = dir.path().join("out");
    let output = run(&source, &out);
    assert_eq!(output.status.code(), Some(0));
    let text = masked(&output);
    assert!(text.contains("Wrote 1 redacted copy"));
    assert!(text.contains("empty.log unchanged"));
    assert_eq!(
        fs::read(out.join("nested/log.txt")).unwrap(),
        b"\xef\xbb\xbfheader\r\n[REDACTED:EMAIL] [REDACTED:CARD]\r\n"
    );
    assert!(!out.join("empty.log").exists());
    assert_eq!(sha256(&input), before);
    assert_eq!(fs::read(&input).unwrap(), original);
}
#[test]
fn file_input_and_lossy_refusal() {
    let dir = dir();
    let source = dir.path().join("log");
    fs::write(&source, b"demo@example.com").unwrap();
    let out = dir.path().join("out");
    let output = run(&source, &out);
    assert_eq!(output.status.code(), Some(0));
    masked(&output);
    assert_eq!(fs::read(out.join("log")).unwrap(), b"[REDACTED:EMAIL]");
    fs::write(&source, b"demo@example.com\xff").unwrap();
    let out = dir.path().join("lossy-out");
    let output = run(&source, &out);
    assert_eq!(output.status.code(), Some(3));
    let text = masked(&output);
    assert!(text.contains("redact-unsafe-lossy"));
    assert!(text.contains("Coverage incomplete"));
    assert!(!out.join("log").exists());
}
#[test]
fn nonempty_output_and_nested_paths_are_usage_errors_without_writes() {
    let dir = dir();
    let source = dir.path().join("input");
    fs::create_dir(&source).unwrap();
    fs::write(source.join("log"), b"demo@example.com").unwrap();
    let out = dir.path().join("out");
    fs::create_dir(&out).unwrap();
    fs::write(out.join("existing"), b"original").unwrap();
    for target in [&out, &source.join("nested"), dir.path(), &source] {
        let output = run(&source, target);
        assert_eq!(output.status.code(), Some(2));
        masked(&output);
    }
    assert_eq!(fs::read(out.join("existing")).unwrap(), b"original");
    assert!(!source.join("nested").exists());
}
#[test]
fn empty_output_is_allowed_and_scan_failure_is_incomplete() {
    let dir = dir();
    let source = dir.path().join("input");
    fs::create_dir(&source).unwrap();
    fs::write(source.join("broken.png"), b"\x89PNG\r\n\x1a\n").unwrap();
    let out = dir.path().join("out");
    fs::create_dir(&out).unwrap();
    let output = run(&source, &out);
    assert_eq!(output.status.code(), Some(3));
    let text = masked(&output);
    assert!(text.contains("image-decode-failed"));
    assert!(text.contains("Coverage incomplete"));
    assert_eq!(fs::read_dir(out).unwrap().count(), 0);
}
#[test]
#[ignore = "requires local OCR models and CPU runtime; set HUSHPUSH_MODELS"]
fn model_screenshot_redacts_rescans_and_preserves_input_sha256() {
    let models = std::env::var_os("HUSHPUSH_MODELS").expect("set HUSHPUSH_MODELS");
    let dir = dir();
    let source = dir.path().join("demo.png");
    fs::write(&source, include_bytes!("fixtures/demo.png")).unwrap();
    let before = sha256(&source);
    let out = dir.path().join("out");
    let output = Command::new(assert_cmd::cargo::cargo_bin!("hushpush"))
        .arg("--models")
        .arg(&models)
        .arg("redact")
        .arg(&source)
        .arg("--out")
        .arg(&out)
        .arg("--include-reserved-domains")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0));
    assert!(masked(&output).contains("Wrote 1 redacted copy"));
    let scan = Command::new(assert_cmd::cargo::cargo_bin!("hushpush"))
        .arg("--models")
        .arg(&models)
        .args(["scan", "dir"])
        .arg(&out)
        .arg("--include-reserved-domains")
        .output()
        .unwrap();
    assert_eq!(scan.status.code(), Some(0));
    let text = masked(&scan);
    assert!(text.contains("No findings in supported categories."));
    assert!(text.contains("Skipped 0"));
    assert_eq!(sha256(&source), before);
    assert_eq!(
        fs::read(&source).unwrap(),
        include_bytes!("fixtures/demo.png")
    );
}
