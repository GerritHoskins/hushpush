//! Score synthetic spans and detector severity without emitting detected content.
use leaklens_core::detect::{ClipboardDetector, ClipboardFieldDetector, Detector, Severity};
use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Deserialize)]
struct Expected {
    start: usize,
    end: usize,
    detector: String,
    severity: String,
}

#[derive(Deserialize)]
struct Case {
    format: String,
    text: String,
    spans: Vec<Expected>,
}

fn groups(format: &str, severity: &str) -> [String; 4] {
    [
        "overall".into(),
        format!("format/{format}"),
        format!("severity/{severity}"),
        format!("format/{format}/severity/{severity}"),
    ]
}

fn main() {
    let path = std::env::args_os()
        .nth(1)
        .expect("synthetic corpus path required");
    let bytes = std::fs::read(path).expect("cannot read synthetic corpus");
    let cases: Vec<Case> = serde_json::from_slice(&bytes).expect("invalid synthetic labels");
    let mut totals = BTreeMap::<String, [usize; 3]>::new();
    let detectors: [&dyn Detector; 2] = [&ClipboardDetector, &ClipboardFieldDetector];
    for case in cases {
        let mut found = Vec::new();
        for detector in detectors {
            let severity = match detector.severity() {
                Severity::Block => "Block",
                Severity::Warn => "Warn",
            };
            let id = format!("{}@{}", detector.id(), detector.version());
            for actual in detector.find(&case.text) {
                found.push((actual.span, id.clone(), severity));
            }
        }
        for (span, id, severity) in &found {
            let correct = case.spans.iter().any(|expected| {
                expected.start == span.start
                    && expected.end == span.end
                    && expected.detector == *id
                    && expected.severity == *severity
            });
            for group in groups(&case.format, severity) {
                totals.entry(group).or_default()[usize::from(!correct)] += 1;
            }
        }
        for expected in &case.spans {
            let present = found.iter().any(|(span, id, severity)| {
                expected.start == span.start
                    && expected.end == span.end
                    && expected.detector == *id
                    && expected.severity == *severity
            });
            if !present {
                for group in groups(&case.format, &expected.severity) {
                    totals.entry(group).or_default()[2] += 1;
                }
            }
        }
    }
    println!(
        "{}",
        serde_json::to_string(&totals).expect("serializable counts")
    );
}
