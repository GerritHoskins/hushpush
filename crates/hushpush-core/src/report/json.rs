//! The disclosure boundary deliberately copies only approved report fields.
use std::io::{self, Write};

use serde::Serialize;

use super::{Location, Report, finding_preview};
use crate::detect::{Category, Confidence, Severity};

#[derive(Serialize)]
struct ReportDto<'a> {
    schema: &'static str,
    tool: ToolDto,
    mode: &'a str,
    config: ConfigDto<'a>,
    policy: PolicyDto,
    detectors: Vec<DetectorDto>,
    findings: Vec<FindingDto<'a>>,
    coverage: CoverageDto,
    allow: AllowDto<'a>,
    summary: SummaryDto,
    exit_code: u8,
}
#[derive(Serialize)]
struct ToolDto {
    name: &'static str,
    version: &'static str,
}
#[derive(Serialize)]
struct ConfigDto<'a> {
    source: &'static str,
    sha256: Option<&'a str>,
}
#[derive(Serialize)]
struct PolicyDto {
    on_incomplete: &'static str,
    on_unsupported: &'static str,
}
#[derive(Serialize)]
struct DetectorDto {
    id: String,
    version: u32,
    severity: &'static str,
}
#[derive(Serialize)]
struct FindingDto<'a> {
    fingerprint: Option<&'a str>,
    detector: &'a str,
    detector_version: u32,
    category: &'static str,
    severity: &'static str,
    confidence: &'static str,
    source: SourceDto<'a>,
    location: LocationDto,
    preview: String,
    suppressed_by: Option<&'a str>,
}
#[derive(Serialize)]
struct SourceDto<'a> {
    path: String,
    blob_oid: Option<&'a str>,
}
#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
enum LocationDto {
    Text {
        line: usize,
        col_start: usize,
        col_end: usize,
    },
    Bbox {
        x: u32,
        y: u32,
        w: u32,
        h: u32,
    },
}
#[derive(Serialize)]
struct CoverageDto {
    unit: &'static str,
    scanned: usize,
    skipped: Vec<SkippedDto>,
    incomplete: bool,
    unsupported: bool,
    approximate: bool,
    full_history: bool,
}
#[derive(Serialize)]
struct SkippedDto {
    path: String,
    reason: &'static str,
    class: &'static str,
}
#[derive(Serialize)]
struct AllowDto<'a> {
    expired: &'a [String],
    unused: &'a [String],
    config_changed_in_outgoing: bool,
}
#[derive(Serialize)]
struct SummaryDto {
    blocking: usize,
    warning: usize,
    suppressed: usize,
}

fn location_key(location: &Location) -> (u8, u64, u64, u64, u64) {
    match *location {
        Location::Text {
            line,
            col_start,
            col_end,
        } => (0, line as u64, col_start as u64, col_end as u64, 0),
        Location::Bbox { x, y, w, h } => (1, y.into(), x.into(), w.into(), h.into()),
    }
}

impl Report {
    /// Serialize only the public, masked v1 DTO; core findings remain non-serializable.
    pub fn write_json(
        &self,
        mut out: impl Write,
        mode: &str,
        full_history: bool,
    ) -> io::Result<()> {
        if !matches!(mode, "dir" | "staged" | "push" | "commits") {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid report mode",
            ));
        }
        let mut findings: Vec<_> = self
            .findings
            .iter()
            .map(|f| (self.config.masked_path(&f.path), f))
            .collect();
        findings.sort_by(|a, b| {
            a.0.cmp(&b.0)
                .then_with(|| location_key(&a.1.location).cmp(&location_key(&b.1.location)))
                .then_with(|| a.1.detector.cmp(&b.1.detector))
                .then_with(|| a.1.fingerprint.cmp(&b.1.fingerprint))
                .then_with(|| a.1.blob_oid.cmp(&b.1.blob_oid))
        });
        let findings = findings
            .into_iter()
            .map(|(path, finding)| FindingDto {
                fingerprint: finding.fingerprint.as_deref(),
                detector: &finding.detector,
                detector_version: finding.detector_version,
                category: match finding.category {
                    Category::Email => "email",
                    Category::PaymentCard => "payment_card",
                    Category::Phone => "phone",
                    Category::HomePath => "home_path",
                    Category::PrivateKey => "private_key",
                    Category::ClipboardRecord => "clipboard_record",
                    Category::UserPattern => "user_pattern",
                },
                severity: match finding.severity {
                    Severity::Block => "block",
                    Severity::Warn => "warn",
                },
                confidence: match finding.confidence {
                    Confidence::High => "high",
                    Confidence::Medium => "medium",
                    Confidence::Low => "low",
                },
                source: SourceDto {
                    path,
                    blob_oid: finding.blob_oid.as_deref(),
                },
                location: match finding.location {
                    Location::Text {
                        line,
                        col_start,
                        col_end,
                    } => LocationDto::Text {
                        line,
                        col_start,
                        col_end,
                    },
                    Location::Bbox { x, y, w, h } => LocationDto::Bbox { x, y, w, h },
                },
                // Never trust caller-supplied preview text.
                preview: finding_preview(finding),
                suppressed_by: finding.suppressed_by.as_deref(),
            })
            .collect();
        let detectors = self
            .config
            .detector_list()
            .into_iter()
            .filter_map(|d| {
                self.config
                    .severity_for(d.as_ref())
                    .map(|severity| DetectorDto {
                        id: d.id().to_owned(),
                        version: d.version(),
                        severity: if severity == Severity::Block {
                            "block"
                        } else {
                            "warn"
                        },
                    })
            })
            .collect();
        let blocking = self
            .findings
            .iter()
            .filter(|f| f.suppressed_by.is_none() && f.severity == Severity::Block)
            .count();
        let warning = self
            .findings
            .iter()
            .filter(|f| f.suppressed_by.is_none() && f.severity == Severity::Warn)
            .count();
        let suppressed = self
            .findings
            .iter()
            .filter(|f| f.suppressed_by.is_some())
            .count();
        let dto = ReportDto {
            schema: "hushpush.report/v1",
            tool: ToolDto {
                name: "hushpush",
                version: env!("CARGO_PKG_VERSION"),
            },
            mode,
            config: ConfigDto {
                source: if self.config.origin.is_some() {
                    "file"
                } else {
                    "defaults"
                },
                sha256: self.config.sha256.as_deref(),
            },
            policy: PolicyDto {
                on_incomplete: self.config.policy.on_incomplete.as_str(),
                on_unsupported: self.config.policy.on_unsupported.as_str(),
            },
            detectors,
            findings,
            coverage: CoverageDto {
                unit: if mode == "dir" { "file" } else { "blob" },
                scanned: self.coverage.scanned,
                skipped: self
                    .coverage
                    .skipped
                    .iter()
                    .map(|s| SkippedDto {
                        path: self.config.masked_path(&s.path),
                        reason: s.reason.as_str(),
                        class: if s.reason.unsupported() {
                            "unsupported"
                        } else if s.reason.incomplete() {
                            "incomplete"
                        } else {
                            "info"
                        },
                    })
                    .collect(),
                incomplete: self.coverage.incomplete
                    || self.coverage.skipped.iter().any(|s| s.reason.incomplete()),
                unsupported: self.coverage.skipped.iter().any(|s| s.reason.unsupported()),
                approximate: self.coverage.approximate,
                full_history,
            },
            allow: AllowDto {
                expired: &self.allow_audit.expired,
                unused: &self.allow_audit.unused,
                config_changed_in_outgoing: self.outgoing_config_changed,
            },
            summary: SummaryDto {
                blocking,
                warning,
                suppressed,
            },
            exit_code: self.exit_code() as u8,
        };
        serde_json::to_writer_pretty(&mut out, &dto)
            .map_err(|_| io::Error::other("could not serialize report"))?;
        writeln!(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        Sensitive,
        config::Config,
        report::{Finding, SkipReason, Skipped},
    };

    fn json(report: &Report, mode: &str) -> serde_json::Value {
        let mut bytes = Vec::new();
        report.write_json(&mut bytes, mode, true).unwrap();
        let value = serde_json::from_slice(&bytes).unwrap();
        let schema: serde_json::Value =
            serde_json::from_str(include_str!("../../../../docs/report.schema.json")).unwrap();
        let validator = jsonschema::validator_for(&schema).unwrap();
        assert!(validator.is_valid(&value), "report must conform to schema");
        value
    }

    fn finding(path: &str, location: Location) -> Finding {
        Finding {
            fingerprint: Some("0123456789abcdef".into()),
            suppressed_by: None,
            detector: "email".into(),
            detector_version: 3,
            category: Category::Email,
            severity: Severity::Warn,
            confidence: Confidence::High,
            path: path.into(),
            blob_oid: Some("a".repeat(40)),
            location,
            preview: "unsafe-caller-prose".into(),
            value: Sensitive::new("synthetic-person@example.com".into()),
        }
    }

    #[test]
    fn schema_covers_specific_unsupported_formats() {
        for reason in [
            SkipReason::Pdf,
            SkipReason::ImageFormatUnsupported,
            SkipReason::Archive,
            SkipReason::Video,
        ] {
            let mut report = Report::default();
            report.coverage.skipped.push(Skipped {
                path: "synthetic.dat".into(),
                reason,
            });
            let value = json(&report, "dir");
            assert_eq!(value["coverage"]["skipped"][0]["reason"], reason.as_str());
            assert_eq!(value["coverage"]["skipped"][0]["class"], "unsupported");
        }
    }

    #[test]
    fn schema_covers_modes_defaults_and_locations_without_values() {
        let mut report = Report {
            findings: vec![
                finding(
                    "z.log",
                    Location::Text {
                        line: 2,
                        col_start: 5,
                        col_end: 33,
                    },
                ),
                finding(
                    "z.log",
                    Location::Text {
                        line: 1,
                        col_start: 1,
                        col_end: 29,
                    },
                ),
                finding(
                    "a.png",
                    Location::Bbox {
                        x: 1,
                        y: 2,
                        w: 3,
                        h: 4,
                    },
                ),
            ],
            ..Report::default()
        };
        report.findings[0].suppressed_by = Some("allow:fixtures".into());
        report.findings[1].suppressed_by = Some("value:0123456789abcdef".into());
        for mode in ["dir", "staged", "push", "commits"] {
            let output = json(&report, mode);
            assert_eq!(output["findings"][0]["source"]["path"], "a.png");
            assert_eq!(output["findings"][1]["location"]["line"], 1);
            assert_eq!(output["summary"]["suppressed"], 2);
            assert_eq!(output["summary"]["warning"], 1);
            assert_eq!(
                output["coverage"]["unit"],
                if mode == "dir" { "file" } else { "blob" }
            );
            let text = output.to_string();
            assert!(!text.contains("synthetic-person"));
            assert!(!text.contains("unsafe-caller-prose"));
            assert!(!text.contains("value\":"));
        }
    }

    #[test]
    fn schema_covers_config_skip_classes_and_nulls() {
        let mut config = Config::parse("[detectors.email]\nseverity='off'\n").unwrap();
        config.origin = Some("private-config-path".into());
        config.sha256 = Some("b".repeat(64));
        let mut report = Report::with_config(&config);
        report.coverage.skipped = vec![
            Skipped {
                path: "a.bin".into(),
                reason: SkipReason::Binary,
            },
            Skipped {
                path: "b.log".into(),
                reason: SkipReason::PartialDecode,
            },
            Skipped {
                path: "c.link".into(),
                reason: SkipReason::Symlink,
            },
        ];
        report.findings.push(finding(
            "synthetic-person@example.com.log",
            Location::Text {
                line: 1,
                col_start: 1,
                col_end: 29,
            },
        ));
        report.findings[0].fingerprint = None;
        report.findings[0].blob_oid = None;
        report.findings.push(finding(
            "a.log",
            Location::Text {
                line: 1,
                col_start: 1,
                col_end: 29,
            },
        ));
        let value = json(&report, "dir");
        assert_eq!(value["findings"][0]["source"]["path"], "[REDACTED]");
        assert_eq!(value["findings"][1]["source"]["path"], "a.log");
        assert_eq!(value["config"]["source"], "file");
        assert_eq!(value["detectors"].as_array().unwrap().len(), 6);
        assert_eq!(value["coverage"]["skipped"][0]["class"], "unsupported");
        assert_eq!(value["coverage"]["skipped"][1]["class"], "incomplete");
        assert_eq!(value["coverage"]["skipped"][2]["class"], "info");
        assert_eq!(value["exit_code"], 3);
        assert!(!value.to_string().contains("private-config-path"));
        assert!(!value.to_string().contains("synthetic-person"));
    }

    #[test]
    fn schema_rejects_extra_fields_at_every_object_boundary() {
        let schema: serde_json::Value =
            serde_json::from_str(include_str!("../../../../docs/report.schema.json")).unwrap();
        fn check(value: &serde_json::Value) {
            if let Some(object) = value.as_object() {
                if object.get("type").is_some_and(|kind| kind == "object") {
                    assert_eq!(
                        object.get("additionalProperties"),
                        Some(&serde_json::Value::Bool(false))
                    );
                }
                object.values().for_each(check);
            } else if let Some(array) = value.as_array() {
                array.iter().for_each(check);
            }
        }
        check(&schema);
        let mut value = json(&Report::default(), "dir");
        value["sensitive"] = serde_json::json!("synthetic-marker");
        assert!(!jsonschema::validator_for(&schema).unwrap().is_valid(&value));
    }
}
