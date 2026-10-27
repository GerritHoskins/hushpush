//! Synthetic disclosure gate across every reporting entry point.
use assert_cmd::Command;
use leaklens_core::{
    config::Config,
    ocr::{OcrChar, OcrEngine, OcrError, OcrLine},
    report::{Location, SkipReason},
    source,
};
use serde_json::Value;
use std::{fs, path::Path, process::Output};
use tempfile::{Builder, TempDir};

const TEXT_EMAIL: &str = "SYNTHETIC-TEXT@LEAKLENS.COMPANY";
const IMAGE_EMAIL: &str = "SYNTHETIC-IMAGE@LEAKLENS.COMPANY";
const LOSSY_EMAIL: &str = "SYNTHETIC-LOSSY@LEAKLENS.COMPANY";
const SCOPE_EMAIL: &str = "SYNTHETIC-SCOPE@LEAKLENS.COMPANY";
const TEXT_CARD: &str = "4242 4242 4242 4242";
const VALUE_CARD: &str = "4111-1111-1111-1111";
const INTERMEDIATE_CARD: &str = "5555 5555 5555 4444";
const PLANTED: [&str; 8] = [
    TEXT_EMAIL,
    IMAGE_EMAIL,
    LOSSY_EMAIL,
    SCOPE_EMAIL,
    TEXT_CARD,
    VALUE_CARD,
    INTERMEDIATE_CARD,
    "demo@example.com",
];

fn private(bytes: &[u8]) {
    for planted in PLANTED {
        let lowercase = planted.to_lowercase();
        let compact: String = planted.chars().filter(|c| c.is_alphanumeric()).collect();
        let digits: String = planted.chars().filter(char::is_ascii_digit).collect();
        for variant in [
            planted.to_owned(),
            lowercase,
            compact.to_lowercase(),
            compact,
            digits,
        ] {
            if !variant.is_empty() {
                assert!(
                    !bytes
                        .windows(variant.len())
                        .any(|window| window == variant.as_bytes()),
                    "a planted synthetic value escaped a disclosure boundary"
                );
            }
        }
    }
}

fn private_state(root: &Path) {
    fn walk(path: &Path) {
        for entry in fs::read_dir(path).unwrap() {
            let entry = entry.unwrap();
            let kind = entry.file_type().unwrap();
            assert!(!kind.is_symlink(), "unexpected local-state symlink");
            if kind.is_dir() {
                walk(&entry.path());
            } else {
                private(&fs::read(entry.path()).unwrap());
            }
        }
    }
    let state = root.join(".git/leaklens");
    if state.exists() {
        walk(&state);
    }
}

fn validate(bytes: &[u8]) -> Value {
    private(bytes);
    let schema: Value = serde_json::from_str(include_str!("../../../docs/report.schema.json"))
        .expect("report schema must parse");
    let validator = jsonschema::validator_for(&schema).expect("report schema must compile");
    let value: Value = serde_json::from_slice(bytes).expect("report must be JSON");
    assert!(validator.is_valid(&value), "report violates JSON v1 schema");
    value
}

fn git(root: &Path, args: &[&str]) -> String {
    let output = std::process::Command::new("git")
        .current_dir(root)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env(
            "GIT_CONFIG_GLOBAL",
            if cfg!(windows) { "NUL" } else { "/dev/null" },
        )
        .args([
            "-c",
            "user.name=Synthetic",
            "-c",
            "user.email=fixture@example.com",
            "-c",
            "commit.gpgsign=false",
            "-c",
            "core.hooksPath=disabled-synthetic-hooks",
        ])
        .args(args)
        .output()
        .unwrap();
    private(&output.stdout);
    private(&output.stderr);
    assert!(
        output.status.success(),
        "synthetic Git fixture operation failed"
    );
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}

fn commit(root: &Path) -> String {
    git(root, &["add", "--all"]);
    git(root, &["commit", "-qm", "synthetic report fixture"]);
    git(root, &["rev-parse", "HEAD"])
}

fn fixture() -> TempDir {
    let target = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/report-leak-fixtures");
    fs::create_dir_all(&target).unwrap();
    let temp = Builder::new()
        .prefix("synthetic-")
        .tempdir_in(target)
        .unwrap();
    git(temp.path(), &["init", "-q", "-b", "main"]);
    fs::create_dir(temp.path().join("input")).unwrap();
    fs::create_dir(temp.path().join("reports")).unwrap();
    fs::write(temp.path().join(".gitignore"), "reports/\n").unwrap();
    fs::write(
        temp.path().join(".leaklens.toml"),
        "[detectors.email]\nseverity = \"block\"\n\n[[allow]]\nid = \"synthetic-scope\"\npaths = [\"**/scope.log\"]\nreason = \"Synthetic fixture\"\n",
    ).unwrap();
    temp
}

fn cli(root: &Path, args: &[&str], input: &str) -> Output {
    Command::new(assert_cmd::cargo::cargo_bin!("leaklens"))
        .current_dir(root)
        .arg("-C")
        .arg(root)
        .args(args)
        .write_stdin(input)
        .output()
        .unwrap()
}

fn run_report(root: &Path, args: &[&str], input: &str, name: &str, mode: &str) -> Value {
    let path = root.join("reports").join(name);
    let mut arguments = args.to_vec();
    arguments.extend(["--json", path.to_str().unwrap()]);
    let output = cli(root, &arguments, input);
    private(&output.stdout);
    private(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(1),
        "expected blocking synthetic findings"
    );
    assert!(!output.stdout.is_empty(), "human report must still print");
    let report = validate(&fs::read(path).unwrap());
    assert_eq!(report["mode"], mode);
    assert_eq!(report["exit_code"], 1);
    private_state(root);
    report
}

struct MockOcr;
impl OcrEngine for MockOcr {
    fn recognize(&self, image: &image::RgbImage) -> Result<Vec<OcrLine>, OcrError> {
        assert!(image.width() > 0 && image.height() > 0);
        Ok(vec![OcrLine {
            text: IMAGE_EMAIL.to_owned(),
            chars: IMAGE_EMAIL
                .chars()
                .enumerate()
                .map(|(i, ch)| OcrChar {
                    ch,
                    bbox: [i as f32, 0.0, (i + 1) as f32, 1.0],
                })
                .collect(),
        }])
    }
}

#[test]
fn every_mode_and_local_state_keep_planted_values_private() {
    let fixture = fixture();
    let root = fixture.path();
    let base = commit(root);
    git(root, &["update-ref", "refs/remotes/origin/main", &base]);
    fs::write(
        root.join("input/text.log"),
        format!("{TEXT_EMAIL}\n{TEXT_CARD}\n"),
    )
    .unwrap();
    fs::write(root.join("input/scope.log"), SCOPE_EMAIL).unwrap();
    fs::write(root.join("input/value.log"), VALUE_CARD).unwrap();
    let mut lossy = LOSSY_EMAIL.as_bytes().to_vec();
    lossy.extend_from_slice(b"\n\xff\n");
    fs::write(root.join("input/lossy.log"), lossy).unwrap();
    fs::write(
        root.join("input/screenshot.png"),
        include_bytes!("fixtures/demo.png"),
    )
    .unwrap();

    let first = run_report(root, &["scan", "dir", "input"], "", "first.json", "dir");
    let fp = first["findings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|finding| finding["source"]["path"] == "value.log")
        .and_then(|finding| finding["fingerprint"].as_str())
        .expect("synthetic value must have a fingerprint");
    let allowed = cli(
        root,
        &["allow", "value", fp, "--reason", "Synthetic fixture"],
        "",
    );
    private(&allowed.stdout);
    private(&allowed.stderr);
    assert!(allowed.status.success(), "value allow must succeed");
    private_state(root);

    let config = Config::load(None, root, false).unwrap();
    let mut report = source::scan_dir_with_image(
        &root.join("input"),
        &config,
        &|bytes, path, origin, reserved| {
            let image = leaklens_core::ocr::decode(bytes)?;
            let dimensions = image.dimensions();
            let lines = MockOcr
                .recognize(&image)
                .map_err(|_| SkipReason::OcrUnavailable)?;
            leaklens_core::ocr::findings(&lines, dimensions, path, origin, reserved)
        },
    )
    .unwrap();
    let store = leaklens_core::allow::LocalStore::discover(root, false)
        .unwrap()
        .unwrap();
    report.apply_allowlists(Some(&store), false);
    assert!(
        report
            .findings
            .iter()
            .any(|finding| matches!(finding.location, Location::Bbox { .. }))
    );
    assert!(
        report
            .coverage
            .skipped
            .iter()
            .any(|skipped| skipped.reason == SkipReason::PartialDecode)
    );
    assert!(
        report
            .findings
            .iter()
            .any(|finding| finding.suppressed_by.as_deref() == Some("allow:synthetic-scope"))
    );
    assert!(
        report
            .findings
            .iter()
            .any(|finding| finding.suppressed_by.as_deref() == Some(&format!("value:{fp}")))
    );
    let mut stdout = Vec::new();
    report.write_terminal(&mut stdout).unwrap();
    private(&stdout);
    let mut json = Vec::new();
    report.write_json(&mut json, "dir", false).unwrap();
    fs::write(root.join("reports/mock-dir.json"), &json).unwrap();
    let json = validate(&fs::read(root.join("reports/mock-dir.json")).unwrap());
    assert_eq!(json["summary"]["suppressed"], 2);
    private_state(root);

    let dir = run_report(root, &["scan", "dir", "input"], "", "dir.json", "dir");
    assert_eq!(dir["summary"]["suppressed"], 2);
    git(root, &["add", "input"]);
    let staged = run_report(root, &["scan", "staged"], "", "staged.json", "staged");
    assert_eq!(staged["summary"]["suppressed"], 2);
    fs::write(root.join("input/intermediate.log"), INTERMEDIATE_CARD).unwrap();
    commit(root);
    fs::write(
        root.join("input/intermediate.log"),
        "ordinary replacement\n",
    )
    .unwrap();
    let head = commit(root);
    let range = format!("{base}..{head}");
    run_report(
        root,
        &["scan", "commits", &range],
        "",
        "commits.json",
        "commits",
    );
    run_report(
        root,
        &["scan", "push", "--remote", "origin"],
        "",
        "push.json",
        "push",
    );
    let input = format!("refs/heads/main {head} refs/heads/main {base}\n");
    let hook = run_report(
        root,
        &["hook", "pre-push", "origin", "synthetic-url"],
        &input,
        "hook.json",
        "push",
    );
    let intermediate = hook["findings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|finding| finding["source"]["path"] == "input/intermediate.log")
        .expect("hook must scan the value removed by the final commit");
    assert_eq!(intermediate["detector"], "card");
    assert!(!intermediate["source"]["blob_oid"].is_null());

    let output = cli(root, &["scan", "commits", &range, "--json", "-"], "");
    private(&output.stdout);
    private(&output.stderr);
    assert_eq!(output.status.code(), Some(1));
    assert!(
        !output.stderr.is_empty(),
        "human report must move to stderr"
    );
    validate(&output.stdout);
    private_state(root);
}
