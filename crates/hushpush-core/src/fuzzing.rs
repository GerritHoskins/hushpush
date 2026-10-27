//! Unstable harness entry points; absent from ordinary product builds.

use crate::{Sensitive, config::Config, detect::Detector, extract, report::Finding};

fn config() -> &'static Config {
    static CONFIG: std::sync::LazyLock<Config> = std::sync::LazyLock::new(|| {
        let mut config = Config::default();
        config.detectors.email.include_reserved_domains = true;
        config.detectors.phone.include_reserved = true;
        config.user_patterns = crate::patterns::parse(
            "version = 1\n[[pattern]]\nid = 'synthetic'\nliterals = ['Synthetic Canary']\nseverity = 'block'",
        )
        .expect("static synthetic pattern");
        config
    });
    &CONFIG
}

fn findings(text: &str) -> Vec<Finding> {
    let locations = extract::TextLocations::new(text);
    let mut output = Vec::new();
    for detector in config().detector_list() {
        for matched in detector.find(text) {
            output.push(Finding {
                fingerprint: None,
                suppressed_by: None,
                detector: detector.id().to_owned(),
                detector_version: detector.version(),
                category: detector.category(),
                severity: detector.severity(),
                confidence: matched.confidence,
                path: "synthetic".to_owned(),
                blob_oid: None,
                location: locations.location(matched.span.clone()),
                preview: String::new(),
                value: Sensitive::new(text[matched.span].to_owned()),
            });
        }
    }
    output
}

pub fn classify_decode_detect(data: &[u8]) {
    let _ = crate::classify::classify(data);
    let (text, _) = extract::decode_text(data);
    let _ = findings(&text);
}

pub fn image_decode(data: &[u8]) {
    let _ = crate::ocr::decode_rgba_with_limits(data, &Config::default().limits);
}

pub fn config_parse(data: &[u8]) {
    if let Ok(text) = std::str::from_utf8(data) {
        let _ = Config::parse(text);
        if let Ok(patterns) = crate::patterns::parse(text) {
            for pattern in patterns {
                let _ = pattern.find(text);
            }
        }
    }
}

pub fn git_parsers(data: &[u8]) {
    crate::source::git::fuzz_parsers(data);
}

pub fn redact_text(data: &[u8]) {
    let (text, partial) = extract::decode_text(data);
    if partial {
        return;
    }
    let found = findings(&text);
    let output =
        crate::redact::fuzz_redact_text(data, &found, config()).expect("valid detector locations");
    let (redacted, partial) = extract::decode_text(&output);
    assert!(!partial, "redaction changed text encoding");
    for finding in found {
        assert!(
            !redacted.contains(finding.value.expose()),
            "matched value retained after redaction"
        );
    }
    assert!(findings(&redacted).is_empty(), "residual redaction finding");
}

#[cfg(test)]
mod tests {
    #[test]
    fn empty_input_regression_initializes_all_detectors() {
        super::classify_decode_detect(b"");
        super::redact_text(b"");
    }

    #[test]
    fn minimized_redaction_residual_regression() {
        super::redact_text(include_bytes!(
            "../../../fuzz/corpus/redact_text/residual-regression"
        ));
    }
}
