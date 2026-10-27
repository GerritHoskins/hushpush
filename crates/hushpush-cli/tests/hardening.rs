use hushpush_core::{
    config::Config,
    report::{Location, Report},
    source::{Origin, scan_bytes},
};

#[test]
#[ignore = "release performance regression: cargo test --release --test hardening -- --ignored"]
fn huge_text_locations_complete_under_ten_seconds() {
    let mut text = String::with_capacity(20_000_000);
    for _ in 0..10_000 {
        text.push_str("synthetic@example.com");
        text.push(' ');
        text.push_str(&"x".repeat(1977));
        text.push('\n');
    }
    assert_eq!(text.len(), 20_000_000);
    let mut config = Config::default();
    config.detectors.email.include_reserved_domains = true;
    let mut report = Report::with_config(&config);
    let started = std::time::Instant::now();
    scan_bytes(
        &mut report,
        "synthetic.log".into(),
        text.as_bytes(),
        Origin::default(),
        true,
    );
    assert_eq!(report.findings.len(), 10_000);
    assert!(
        started.elapsed().as_secs_f64() < 10.0,
        "20 MB/10,000 finding scan exceeded 10 seconds"
    );
    assert!(matches!(
        report.findings.last().unwrap().location,
        Location::Text {
            line: 10_000,
            col_start: 1,
            ..
        }
    ));
}

#[test]
#[ignore = "large release regression"]
fn thirty_mb_single_line_has_scalar_columns() {
    let mut text = "x".repeat(30_000_000 - 23);
    text.push_str(" synthetic@example.com ");
    assert_eq!(text.len(), 30_000_000);
    let mut report = Report::default();
    scan_bytes(
        &mut report,
        "synthetic.log".into(),
        text.as_bytes(),
        Origin::default(),
        true,
    );
    assert_eq!(report.coverage.scanned, 1);
    assert_eq!(report.findings.len(), 1);
    assert!(matches!(
        report.findings[0].location,
        Location::Text { line: 1, .. }
    ));
}
