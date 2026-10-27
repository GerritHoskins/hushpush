use assert_cmd::Command;

fn leaklens() -> Command {
    Command::new(assert_cmd::cargo::cargo_bin!("leaklens"))
}

#[test]
fn help_succeeds() {
    let output = leaklens()
        .arg("--help")
        .assert()
        .success()
        .get_output()
        .clone();
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("Usage: leaklens"));
    assert!(!stdout.contains("debug-panic"));
    assert!(output.stderr.is_empty());
}

#[test]
fn redact_missing_source_exits_two_without_echoing_arguments() {
    let output = leaklens()
        .args(["redact", "synthetic-path", "--out", "synthetic-output"])
        .assert()
        .code(2)
        .get_output()
        .clone();
    assert!(output.stdout.is_empty());
    assert_eq!(
        String::from_utf8(output.stderr).unwrap().trim_end(),
        "leaklens: invalid redaction source or output directory"
    );
}

#[test]
fn usage_errors_exit_two() {
    for args in [
        vec![],
        vec!["unknown-command"],
        vec!["scan", "dir"],
        vec!["scan", "commits"],
        vec!["scan", "push", "--remote"],
        vec!["redact", "synthetic-path"],
        vec!["hook", "pre-push", "synthetic-remote"],
    ] {
        leaklens().args(args).assert().code(2);
    }
}

#[cfg(debug_assertions)]
#[test]
fn panic_hook_never_discloses_payload_even_with_debug_or_backtrace() {
    for debug in [None, Some("0"), Some("1")] {
        let mut command = leaklens();
        command
            .arg("debug-panic")
            .env("RUST_BACKTRACE", "full")
            .env_remove("LEAKLENS_DEBUG");
        if let Some(value) = debug {
            command.env("LEAKLENS_DEBUG", value);
        }
        let output = command.assert().code(4).get_output().clone();
        assert!(output.stdout.is_empty());
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(!stderr.contains("synthetic-panic-payload-must-stay-hidden"));
        let mut lines = stderr.lines();
        assert_eq!(lines.next(), Some("leaklens: internal error (exit 4)"));
        if debug == Some("1") {
            assert!(lines.next().unwrap().starts_with("at "));
        }
        assert_eq!(lines.next(), None);
    }
}
