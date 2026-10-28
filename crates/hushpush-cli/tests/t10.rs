use assert_cmd::Command;
use std::{
    fs,
    path::{Path, PathBuf},
    process::Output,
};
use tempfile::{Builder, TempDir};

const PHONE: &str = "+49 30 87654321";
const RESERVED_PHONE: &str = "+1 212 555-0107";
const HOME: &str = "/Users/SyntheticPerson/Documents/";
const PEM: &str =
    "-----BEGIN PRIVATE KEY-----\nU3ludGhldGljS2V5T25seQ==\n-----END PRIVATE KEY-----";
const CLIPBOARD: &str = "clipboard=\"synthetic private payload\"";
const USER_VALUE: &str = "Synthetic Family";
const PATTERN: &str =
    "version=1\n[[pattern]]\nid='family-names'\nliterals=['Synthetic Family']\nseverity='block'\n";

struct Fixture {
    _temp: TempDir,
    root: PathBuf,
    patterns: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let temp = Builder::new()
            .prefix("synthetic-t10-")
            .tempdir_in(env!("CARGO_MANIFEST_DIR"))
            .unwrap();
        let root = temp.path().join("repo");
        fs::create_dir(&root).unwrap();
        fs::create_dir(root.join("input")).unwrap();
        git(&root, &["init", "-q", "-b", "main"]);
        let patterns = temp.path().join("patterns.toml");
        fs::write(&patterns, "version=1\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&patterns, fs::Permissions::from_mode(0o600)).unwrap();
        }
        Self {
            _temp: temp,
            root,
            patterns,
        }
    }
    fn command(&self) -> Command {
        let mut command = Command::new(assert_cmd::cargo::cargo_bin!("hushpush"));
        command
            .current_dir(&self.root)
            .arg("-C")
            .arg(&self.root)
            .env("HUSHPUSH_PATTERNS", &self.patterns);
        command
    }
    fn scan(&self, args: &[&str], code: i32) -> serde_json::Value {
        let output = self
            .command()
            .args(args)
            .args(["scan", "dir", "input", "--json", "-"])
            .output()
            .unwrap();
        checked(&output, code);
        serde_json::from_slice(&output.stdout).unwrap()
    }
    fn input(&self, text: &str) {
        fs::write(self.root.join("input/log.txt"), text).unwrap();
    }
    fn config(&self, text: &str) {
        fs::write(self.root.join(".hushpush.toml"), text).unwrap();
    }
}

fn safe(output: &Output) {
    for bytes in [&output.stdout, &output.stderr] {
        let text = String::from_utf8_lossy(bytes);
        for value in [
            PHONE,
            RESERVED_PHONE,
            "SyntheticPerson",
            "U3ludGhldGljS2V5T25seQ==",
            "synthetic private payload",
            USER_VALUE,
            "private-marker-value",
        ] {
            assert!(!text.contains(value), "synthetic sensitive value disclosed");
        }
    }
}

fn checked(output: &Output, code: i32) {
    safe(output);
    assert_eq!(
        output.status.code(),
        Some(code),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn git(root: &Path, args: &[&str]) -> Output {
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
    safe(&output);
    assert!(output.status.success(), "synthetic Git operation failed");
    output
}

#[test]
fn each_new_detector_obeys_block_warn_and_off_configuration() {
    for (id, value, category) in [
        ("phone", PHONE, "phone"),
        ("home_path", HOME, "home_path"),
        ("pem", PEM, "private_key"),
        (
            "clipboard",
            "ClipboardService: text=\"synthetic private payload\"",
            "clipboard_record",
        ),
        ("clipboard_field", CLIPBOARD, "clipboard_record"),
        ("user_pattern", USER_VALUE, "user_pattern"),
    ] {
        let fixture = Fixture::new();
        fixture.input(value);
        if id == "user_pattern" {
            fs::write(&fixture.patterns, PATTERN).unwrap();
        }
        for (severity, code) in [("block", 1), ("warn", 0), ("off", 0)] {
            fixture.config(&format!("[detectors.{id}]\nseverity='{severity}'\n"));
            let report = fixture.scan(&[], code);
            let findings = report["findings"].as_array().unwrap();
            if severity == "off" {
                assert!(findings.is_empty());
            } else {
                assert_eq!(findings.len(), 1, "expected exactly one configured finding");
                assert_eq!(findings[0]["category"], category);
                assert_eq!(findings[0]["severity"], severity);
            }
        }
    }
}

#[test]
fn deprecated_clipboard_config_alias_maps_to_fields_and_emits_one_note() {
    let fixture = Fixture::new();
    fixture.input(CLIPBOARD);
    for (setting, code) in [("warn", 0), ("block", 1), ("off", 0)] {
        fixture.config(&format!(
            "[detectors.clipboard_loose]\nseverity='{setting}'\n"
        ));
        let output = fixture
            .command()
            .args(["scan", "dir", "input", "--json", "-"])
            .output()
            .unwrap();
        checked(&output, code);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(
            stderr
                .lines()
                .filter(|line| line.contains("is deprecated"))
                .count(),
            1
        );
        let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        let findings = report["findings"].as_array().unwrap();
        if setting == "off" {
            assert!(findings.is_empty());
        } else {
            assert_eq!(findings.len(), 1);
            assert_eq!(findings[0]["detector"], "clipboard_field");
            assert_eq!(findings[0]["detector_version"], 1);
            assert_eq!(findings[0]["severity"], setting);
        }
    }
    fixture.config("[detectors.clipboard_loose]\nseverity='off'\n[detectors.clipboard_field]\nseverity='warn'\n");
    let output = fixture
        .command()
        .args(["scan", "dir", "input"])
        .output()
        .unwrap();
    checked(&output, 2);
}

#[test]
fn reserved_flag_alias_and_detector_specific_config_are_supported() {
    let fixture = Fixture::new();
    fixture.input(&format!("{RESERVED_PHONE}\ndemo@example.com\n"));
    assert!(
        fixture.scan(&[], 0)["findings"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    for flag in ["--include-reserved", "--include-reserved-domains"] {
        let report = fixture.scan(&[flag], 0);
        let findings = report["findings"].as_array().unwrap();
        assert_eq!(findings.len(), 2);
        assert!(findings.iter().any(|f| f["category"] == "phone"));
        assert!(findings.iter().any(|f| f["category"] == "email"));
    }
    fixture.config("[detectors.phone]\ninclude_reserved=true\n");
    let report = fixture.scan(&[], 0);
    assert_eq!(report["findings"].as_array().unwrap().len(), 1);
    assert_eq!(report["findings"][0]["category"], "phone");
    fixture.config("[detectors.email]\ninclude_reserved_domains=true\n");
    let report = fixture.scan(&[], 0);
    assert_eq!(report["findings"].as_array().unwrap().len(), 1);
    assert_eq!(report["findings"][0]["category"], "email");
    let help = fixture.command().arg("--help").output().unwrap();
    let help = String::from_utf8_lossy(&help.stdout);
    assert!(help.contains("--include-reserved"));
    assert!(!help.contains("--include-reserved-domains"));
}

#[test]
fn external_patterns_are_private_versioned_and_can_be_disabled() {
    let fixture = Fixture::new();
    fs::write(&fixture.patterns, PATTERN).unwrap();
    fixture.input("prefix SYNTHETIC FAMILY suffix");
    let report = fixture.scan(&[], 1);
    assert_eq!(report["findings"][0]["detector"], "user:family-names");
    assert_eq!(report["findings"][0]["detector_version"], 1);
    assert!(
        report["findings"][0]["preview"]
            .as_str()
            .unwrap()
            .starts_with("[USER:family-names len=")
    );
    assert!(
        report["detectors"]
            .as_array()
            .unwrap()
            .iter()
            .any(|detector| detector["id"] == "user:family-names")
    );
    let output = fixture
        .command()
        .args(["scan", "dir", "input"])
        .output()
        .unwrap();
    checked(&output, 1);
    assert!(String::from_utf8_lossy(&output.stdout).contains("user:family-names@1"));
    assert!(
        fixture.scan(&["--no-user-patterns"], 0)["findings"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}

#[test]
fn warn_patterns_keep_their_default_and_explicit_id_overrides_win() {
    let fixture = Fixture::new();
    fs::write(
        &fixture.patterns,
        PATTERN.replace("severity='block'", "severity='warn'"),
    )
    .unwrap();
    fixture.input(USER_VALUE);
    assert_eq!(fixture.scan(&[], 0)["findings"][0]["severity"], "warn");
    fixture.config("[detectors.user_pattern]\nseverity='block'\n");
    assert_eq!(fixture.scan(&[], 1)["findings"][0]["severity"], "block");
    fixture.config("[detectors.user_pattern]\nseverity='off'\n[detectors.'user:family-names']\nseverity='block'\n");
    assert_eq!(fixture.scan(&[], 1)["findings"][0]["severity"], "block");
    fixture.config("[detectors.'user:family-names']\nseverity='off'\n");
    assert!(
        fixture.scan(&[], 0)["findings"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}

#[test]
fn repository_patterns_and_invalid_external_values_exit_two_without_disclosure() {
    let fixture = Fixture::new();
    fixture.input("ordinary content");
    fixture.config(PATTERN);
    let output = fixture
        .command()
        .args(["scan", "dir", "input"])
        .output()
        .unwrap();
    checked(&output, 2);
    assert!(String::from_utf8_lossy(&output.stderr).contains("pattern"));
    fs::remove_file(fixture.root.join(".hushpush.toml")).unwrap();
    let inside = fixture.root.join("patterns.toml");
    fs::write(&inside, PATTERN).unwrap();
    let output = fixture
        .command()
        .env("HUSHPUSH_PATTERNS", &inside)
        .args(["scan", "dir", "input"])
        .output()
        .unwrap();
    checked(&output, 2);
    assert!(String::from_utf8_lossy(&output.stderr).contains("outside repository"));
    for text in [
        "version='private-marker-value'",
        "version=1\n[[pattern]]\nid='safe'\nregex='[private-marker-value'\nseverity='block'",
        "version=1\n[[pattern]]\nid='safe'\nliterals=['private-marker-value']\nseverity='private-marker-value'",
        "version='private-marker-value",
    ] {
        fs::write(&fixture.patterns, text).unwrap();
        let output = fixture
            .command()
            .args(["scan", "dir", "input"])
            .output()
            .unwrap();
        checked(&output, 2);
    }
    // Disabling patterns must bypass even an invalid external file.
    fixture.scan(&["--no-user-patterns"], 0);
}

fn commit(root: &Path, text: &str) -> String {
    fs::write(root.join("input/log.txt"), text).unwrap();
    git(root, &["add", "input/log.txt"]);
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
            "synthetic fixture",
        ],
    );
    String::from_utf8(git(root, &["rev-parse", "HEAD"]).stdout)
        .unwrap()
        .trim()
        .into()
}

#[test]
fn pre_push_blocks_a_pem_in_an_intermediate_commit() {
    let fixture = Fixture::new();
    let base = commit(&fixture.root, "ordinary base");
    commit(&fixture.root, PEM);
    let tip = commit(&fixture.root, "ordinary replacement");
    let output = fixture
        .command()
        .args(["hook", "pre-push", "synthetic-remote", "synthetic-url"])
        .write_stdin(format!("refs/heads/main {tip} refs/heads/main {base}\n"))
        .output()
        .unwrap();
    checked(&output, 1);
    assert!(String::from_utf8_lossy(&output.stdout).contains("[PRIVATE_KEY len="));
}

#[test]
fn all_new_kinds_redact_and_rescan_with_exact_crlf_bytes() {
    let fixture = Fixture::new();
    fs::write(&fixture.patterns, PATTERN).unwrap();
    fixture.input(&format!("{PHONE}\r\n{HOME}\r\n{}\r\nclipboard=\"synthetic first\r\nsynthetic second\"\r\n{USER_VALUE}\r\n", PEM.replace('\n', "\r\n")));
    let out = fixture.root.join("redacted");
    let output = fixture
        .command()
        .args(["redact", "input", "--out"])
        .arg(&out)
        .output()
        .unwrap();
    checked(&output, 0);
    let bytes = fs::read(out.join("log.txt")).unwrap();
    assert_eq!(bytes, b"[REDACTED:PHONE]\r\n/Users/[REDACTED:HOME_USER]/Documents/\r\n[REDACTED:PRIVATE_KEY]\r\nclipboard=\"[REDACTED:CLIPBOARD]\"\r\n[REDACTED:USER_PATTERN]\r\n");
}

#[test]
fn user_pattern_matches_in_filenames_are_masked_in_reports() {
    let fixture = Fixture::new();
    fs::write(&fixture.patterns, PATTERN).unwrap();
    fs::write(
        fixture.root.join("input").join(format!("{USER_VALUE}.log")),
        USER_VALUE,
    )
    .unwrap();
    let report = fixture.scan(&[], 1);
    assert_eq!(report["findings"][0]["source"]["path"], "[REDACTED].log");
    let output = fixture
        .command()
        .args(["scan", "dir", "input"])
        .output()
        .unwrap();
    checked(&output, 1);
}
