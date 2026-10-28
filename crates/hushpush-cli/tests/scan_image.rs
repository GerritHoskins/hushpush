use assert_cmd::Command;
use std::{fs, path::Path};
use tempfile::Builder;
const SCREENSHOT: &[u8] = include_bytes!("fixtures/demo.png");
fn dir() -> tempfile::TempDir {
    Builder::new()
        .prefix("synthetic-t4-")
        .tempdir_in(env!("CARGO_MANIFEST_DIR"))
        .unwrap()
}
fn scan(path: &Path, models: Option<&Path>) -> std::process::Output {
    let mut command = Command::new(assert_cmd::cargo::cargo_bin!("hushpush"));
    if let Some(models) = models {
        command.arg("--models").arg(models);
    }
    command
        .args(["scan", "dir"])
        .arg(path)
        .arg("--include-reserved-domains");
    command.output().unwrap()
}
fn masked(output: &std::process::Output) -> String {
    assert!(output.stderr.is_empty());
    let text = String::from_utf8(output.stdout.clone()).unwrap();
    for value in [
        "demo@example.com",
        "4242 4242 4242 4242",
        "4242424242424242",
    ] {
        assert!(!text.contains(value), "synthetic value disclosed");
    }
    text
}
#[test]
fn missing_ocr_skips_each_image_once_with_one_hint_and_scans_text() {
    let dir = dir();
    fs::write(dir.path().join("a.png"), SCREENSHOT).unwrap();
    fs::write(dir.path().join("b.png"), SCREENSHOT).unwrap();
    fs::write(dir.path().join("log"), b"demo@example.com").unwrap();
    let output = scan(dir.path(), Some(&dir.path().join("missing-models")));
    assert_eq!(output.status.code(), Some(3));
    let text = masked(&output);
    assert!(text.contains("ocr-unavailable=2"));
    assert!(text.contains("Scanned 1 file"));
    assert!(text.contains("[EMAIL len=16]"));
    assert_eq!(text.matches("OCR unavailable:").count(), 1);
}
#[test]
fn invalid_models_are_incomplete_and_do_not_abort() {
    let dir = dir();
    fs::write(dir.path().join("demo.png"), SCREENSHOT).unwrap();
    for name in [
        "mobile_dict.txt",
        "mobile_det_inference.onnx",
        "mobile_rec_inference.onnx",
    ] {
        fs::write(dir.path().join(name), b"synthetic-invalid-model").unwrap();
    }
    let output = scan(dir.path(), Some(dir.path()));
    assert_eq!(output.status.code(), Some(3));
    assert!(masked(&output).contains("ocr-unavailable=1"));
}
fn model_demo() {
    let models = std::env::var_os("HUSHPUSH_MODELS");
    let dir = dir();
    fs::write(dir.path().join("demo.png"), SCREENSHOT).unwrap();
    let output = scan(dir.path(), models.as_deref().map(Path::new));
    assert_eq!(output.status.code(), Some(1));
    let text = masked(&output);
    assert!(text.contains(":bbox=["));
    assert!(text.contains("Medium  [EMAIL len=16]"));
    assert!(text.contains("Medium  [CARD len=19]"));
    assert!(text.contains("Skipped 0"));
}
#[test]
#[ignore = "requires HUSHPUSH_MODELS or bundled-models"]
fn synthetic_screenshot_has_masked_bbox_findings() {
    model_demo();
}
#[test]
fn synthetic_screenshot_when_models_are_configured() {
    if std::env::var_os("HUSHPUSH_MODELS").is_some() {
        model_demo();
    }
}

#[test]
fn image_file_size_is_checked_before_decode_or_ocr() {
    use std::io::Write;
    let dir = dir();
    let mut file = fs::File::create(dir.path().join("huge.png")).unwrap();
    file.write_all(b"\x89PNG\r\n\x1a\n").unwrap();
    file.set_len(50 * 1024 * 1024 + 1).unwrap();
    drop(file);
    let output = scan(dir.path(), Some(&dir.path().join("missing-models")));
    assert_eq!(output.status.code(), Some(3));
    let text = masked(&output);
    assert!(text.contains("image-too-large=1"));
    assert!(!text.contains("ocr-unavailable"));
}
#[test]
#[ignore = "requires HUSHPUSH_MODELS or bundled-models"]
fn image_pipeline_is_shared_by_staged_commits_and_push() {
    let models = std::env::var_os("HUSHPUSH_MODELS");
    let dir = dir();
    let git = |args: &[&str]| {
        let output = std::process::Command::new("git")
            .current_dir(dir.path())
            .args([
                "-c",
                "user.name=Synthetic",
                "-c",
                "user.email=synthetic@example.com",
                "-c",
                "commit.gpgsign=false",
                "-c",
                "core.hooksPath=disabled-synthetic-hooks",
            ])
            .args(args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "synthetic repository operation failed"
        );
    };
    git(&["init"]);
    git(&["commit", "--allow-empty", "-m", "synthetic-base"]);
    fs::write(dir.path().join("demo.png"), SCREENSHOT).unwrap();
    git(&["add", "demo.png"]);
    let check = |args: &[&str]| {
        let mut command = Command::new(assert_cmd::cargo::cargo_bin!("hushpush"));
        if let Some(models) = &models {
            command.arg("--models").arg(models);
        }
        let output = command
            .arg("-C")
            .arg(dir.path())
            .args(args)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(1));
        let text = masked(&output);
        assert!(text.contains("demo.png:bbox=["));
        assert!(text.contains("Medium  [CARD len=19]"));
        assert!(text.contains("Skipped 0"));
    };
    check(&["scan", "staged"]);
    git(&["commit", "-m", "synthetic-image"]);
    check(&["scan", "commits", "HEAD~1..HEAD"]);
    check(&["scan", "push", "--remote", "synthetic-no-remote"]);
}
