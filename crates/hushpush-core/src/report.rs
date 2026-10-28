//! Masked findings and coverage reports.

mod json;

use std::{
    collections::BTreeMap,
    io::{self, Write},
};

use crate::{
    ExitCode, Sensitive,
    detect::{Category, Confidence, Detector, Severity},
};

#[derive(Clone, Debug)]
pub struct Finding {
    pub fingerprint: Option<String>,
    pub suppressed_by: Option<String>,
    pub detector: String,
    pub detector_version: u32,
    pub category: Category,
    pub severity: Severity,
    pub confidence: Confidence,
    pub path: String,
    pub blob_oid: Option<String>,
    pub location: Location,
    pub preview: String,
    pub value: Sensitive<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Location {
    Bbox {
        x: u32,
        y: u32,
        w: u32,
        h: u32,
    },
    /// Columns are Unicode scalar positions; col_end is exclusive.
    Text {
        line: usize,
        col_start: usize,
        col_end: usize,
    },
}

#[derive(Default, Debug)]
pub struct Coverage {
    pub scanned: usize,
    pub skipped: Vec<Skipped>,
    pub incomplete: bool,
    pub approximate: bool,
}

#[derive(Debug)]
pub struct Skipped {
    pub path: String,
    pub reason: SkipReason,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum SkipReason {
    ImageDecodeFailed,
    ImageTooLarge,
    OcrUnavailable,
    OcrTimeout,
    Binary,
    TooLarge,
    PartialDecode,
    Unreadable,
    Symlink,
    Submodule,
    UnsupportedFileType,
    MissingObject,
    LfsPointer,
    Pdf,
    ImageFormatUnsupported,
    Archive,
    Video,
}

impl SkipReason {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ImageDecodeFailed => "image-decode-failed",
            Self::ImageTooLarge => "image-too-large",
            Self::OcrUnavailable => "ocr-unavailable",
            Self::OcrTimeout => "ocr-timeout",
            Self::Binary => "binary",
            Self::TooLarge => "too-large",
            Self::PartialDecode => "partial-decode",
            Self::Unreadable => "unreadable",
            Self::Symlink => "symlink",
            Self::Submodule => "submodule",
            Self::UnsupportedFileType => "unsupported-file-type",
            Self::MissingObject => "missing-object",
            Self::LfsPointer => "lfs-pointer",
            Self::Pdf => "pdf",
            Self::ImageFormatUnsupported => "image-format-unsupported",
            Self::Archive => "archive",
            Self::Video => "video",
        }
    }

    pub fn incomplete(self) -> bool {
        !self.unsupported() && self != Self::Symlink
    }
    pub fn unsupported(self) -> bool {
        matches!(
            self,
            Self::Binary
                | Self::Submodule
                | Self::UnsupportedFileType
                | Self::LfsPointer
                | Self::Pdf
                | Self::ImageFormatUnsupported
                | Self::Archive
                | Self::Video
        )
    }
}

#[derive(Default, Debug)]
pub struct Report {
    pub allow_audit: AllowAudit,
    pub outgoing_config_changed: bool,
    pub config: crate::config::Config,
    pub findings: Vec<Finding>,
    pub coverage: Coverage,
}

#[derive(Default, Debug)]
pub struct AllowAudit {
    pub has_rules: bool,
    pub suppressed: BTreeMap<String, usize>,
    pub expired: Vec<String>,
    pub unused: Vec<String>,
    pub outside_repository: bool,
}

fn unsafe_display_char(c: char) -> bool {
    c.is_control()
        || matches!(c, '\u{034f}' | '\u{061c}' | '\u{180e}' | '\u{200b}'..='\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2060}'..='\u{206f}' | '\u{fe00}'..='\u{fe0f}' | '\u{feff}' | '\u{00ad}' | '\u{e0000}'..='\u{e007f}' | '\u{e0100}'..='\u{e01ef}')
}

/// Neutralize terminal controls, including complete ANSI escape sequences.
pub fn sanitize_path(path: &str) -> String {
    let mut result = String::new();
    let mut chars = path.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' || c == '\u{9b}' || c == '\u{9d}' {
            result.push('?');
            let kind = if c == '\u{1b}' {
                chars.next()
            } else if c == '\u{9b}' {
                Some('[')
            } else {
                Some(']')
            };
            match kind {
                Some('[') => {
                    for next in chars.by_ref() {
                        if ('@'..='~').contains(&next) {
                            break;
                        }
                    }
                }
                Some(']') => {
                    while let Some(next) = chars.next() {
                        if next == '\u{7}' || next == '\u{9c}' {
                            break;
                        }
                        if next == '\u{1b}' && chars.peek() == Some(&'\\') {
                            chars.next();
                            break;
                        }
                    }
                }
                Some(next) if !unsafe_display_char(next) => {
                    result.push(next);
                }
                Some(_) => result.push('?'),
                None => {}
            }
        } else {
            result.push(if unsafe_display_char(c) { '?' } else { c });
        }
    }
    let email = crate::detect::EmailDetector::new(true);
    let mut spans: Vec<_> = email
        .find(&result)
        .into_iter()
        .chain(crate::detect::CardDetector.find(&result))
        .chain(crate::detect::PhoneDetector::new(true).find(&result))
        .chain(crate::detect::HomePathDetector.find(&result))
        .chain(crate::detect::PemDetector.find(&result))
        .chain(crate::detect::ClipboardDetector.find(&result))
        .chain(crate::detect::ClipboardFieldDetector.find(&result))
        .map(|m| m.span)
        .collect();
    spans.sort_by_key(|s| s.start);
    let mut masked = String::new();
    let mut end = 0;
    for span in spans {
        if span.start >= end {
            masked.push_str(&result[end..span.start]);
            masked.push_str("[REDACTED]");
            end = span.end;
        }
    }
    masked.push_str(&result[end..]);
    masked
}

pub fn preview(category: Category, char_count: usize) -> String {
    let name = match category {
        Category::Email => "EMAIL",
        Category::PaymentCard => "CARD",
        Category::Phone => "PHONE",
        Category::HomePath => "HOME_USER",
        Category::PrivateKey => "PRIVATE_KEY",
        Category::ClipboardRecord => "CLIPBOARD",
        Category::UserPattern => "USER_PATTERN",
    };
    format!("[{name} len={char_count}]")
}

pub fn finding_preview(finding: &Finding) -> String {
    preview_value(finding.category, &finding.detector, finding.value.expose())
}

pub fn preview_value(category: Category, detector: &str, value: &str) -> String {
    let n = value.chars().count();
    match category {
        Category::ClipboardRecord => format!("[CLIPBOARD len={n} lines={}]", value.lines().count()),
        Category::UserPattern => format!(
            "[USER:{} len={n}]",
            sanitize_path(detector.strip_prefix("user:").unwrap_or("pattern"))
        ),
        category => preview(category, n),
    }
}

impl Report {
    pub fn with_config(config: &crate::config::Config) -> Self {
        Self {
            config: config.clone(),
            ..Self::default()
        }
    }
    pub fn apply_allowlists(&mut self, store: Option<&crate::allow::LocalStore>, disabled: bool) {
        let today = chrono::Utc::now().date_naive();
        self.allow_audit = AllowAudit {
            has_rules: !self.config.allow.is_empty()
                || store.is_some_and(|s| !s.values().is_empty()),
            outside_repository: store.is_none(),
            ..AllowAudit::default()
        };
        let mut used = std::collections::BTreeSet::new();
        for finding in &mut self.findings {
            finding.fingerprint = store.map(|s| s.fingerprint(&finding.detector, &finding.value));
            finding.suppressed_by = None;
            if disabled {
                continue;
            }
            if let Some(rule) =
                self.config.allow.iter().find(|rule| {
                    !rule.expired(today) && rule.matches(&finding.path, &finding.detector)
                })
            {
                finding.suppressed_by = Some(format!("allow:{}", rule.id));
                used.insert(rule.id.clone());
                *self
                    .allow_audit
                    .suppressed
                    .entry(rule.id.clone())
                    .or_default() += 1;
            } else if let Some(rule) = store.and_then(|s| {
                s.values()
                    .iter()
                    .find(|rule| !rule.expired() && Some(&rule.fp) == finding.fingerprint.as_ref())
            }) {
                finding.suppressed_by = Some(format!("value:{}", rule.fp));
                used.insert(format!("value:{}", rule.fp));
                *self
                    .allow_audit
                    .suppressed
                    .entry("value".into())
                    .or_default() += 1;
            }
        }
        if disabled {
            return;
        }
        for rule in &self.config.allow {
            if rule.expired(today) {
                self.allow_audit.expired.push(rule.id.clone());
            } else if !used.contains(&rule.id) {
                self.allow_audit.unused.push(rule.id.clone());
            }
        }
        if let Some(store) = store {
            for rule in store.values() {
                let id = format!("value:{}", rule.fp);
                if rule.expired() {
                    self.allow_audit.expired.push(id);
                } else if !used.contains(&id) {
                    self.allow_audit.unused.push(id);
                }
            }
        }
    }

    pub fn write_allow_audit(&self, mut out: impl Write) -> io::Result<()> {
        let total: usize = self.allow_audit.suppressed.values().sum();
        if total > 0 || self.allow_audit.has_rules || !self.config.allow.is_empty() {
            write!(out, "{total} suppressed")?;
            if total > 0 {
                let counts = self
                    .allow_audit
                    .suppressed
                    .iter()
                    .map(|(id, n)| format!("{id}={n}"))
                    .collect::<Vec<_>>()
                    .join(", ");
                write!(out, " ({counts})")?;
            }
            writeln!(out)?;
        }
        if !self.allow_audit.expired.is_empty() {
            writeln!(
                out,
                "Expired allow rules ignored: {}",
                self.allow_audit.expired.join(", ")
            )?;
        }
        if !self.allow_audit.unused.is_empty() {
            writeln!(
                out,
                "Unused allow rules: {}",
                self.allow_audit.unused.join(", ")
            )?;
        }
        if self.allow_audit.outside_repository {
            writeln!(
                out,
                "Outside a Git repository: fingerprints and value allows are unavailable."
            )?;
        }
        if self.outgoing_config_changed {
            writeln!(
                out,
                "Notice: outgoing commits modify .hushpush.toml (allow rules may have changed)"
            )?;
        }
        Ok(())
    }

    fn policy(&self, reason: SkipReason) -> Option<crate::config::PolicyAction> {
        if reason.unsupported() {
            Some(self.config.policy.on_unsupported)
        } else if reason.incomplete() {
            Some(self.config.policy.on_incomplete)
        } else {
            None
        }
    }

    pub fn exit_code(&self) -> ExitCode {
        if self
            .findings
            .iter()
            .any(|finding| finding.severity == Severity::Block && finding.suppressed_by.is_none())
        {
            ExitCode::BlockingFindings
        } else if self
            .coverage
            .skipped
            .iter()
            .any(|s| self.policy(s.reason) == Some(crate::config::PolicyAction::Block))
        {
            ExitCode::IncompleteCoverage
        } else {
            ExitCode::NoBlockingFindings
        }
    }

    pub fn write_metadata(&self, mut out: impl Write) -> io::Result<()> {
        write!(out, "Detectors:")?;
        let mut separator = " ";
        for detector in self.config.detector_list() {
            if let Some(severity) = self.config.severity_for(detector.as_ref()) {
                write!(
                    out,
                    "{separator}{}@{} ({})",
                    detector.id(),
                    detector.version(),
                    if severity == Severity::Block {
                        "block"
                    } else {
                        "warn"
                    }
                )?;
                separator = ", ";
            }
        }
        if separator == " " {
            write!(out, " none")?;
        }
        writeln!(out)?;
        writeln!(
            out,
            "Config: {}",
            self.config
                .masked_path(self.config.origin.as_deref().unwrap_or("defaults"))
        )?;
        Ok(())
    }

    pub fn write_findings(&self, mut out: impl Write) -> io::Result<()> {
        for finding in &self.findings {
            let location = match finding.location {
                Location::Text {
                    line, col_start, ..
                } => format!(":{line}:{col_start}"),
                Location::Bbox { x, y, w, h } => format!(":bbox=[{x},{y},{w},{h}]"),
            };
            // Recompute the preview so callers cannot inject value-bearing prose.
            writeln!(
                out,
                "{}{location}  {:?}  {:?}  {:?}  {}{}{}",
                self.config.masked_path(&finding.path),
                finding.severity,
                finding.category,
                finding.confidence,
                finding_preview(finding),
                finding
                    .fingerprint
                    .as_ref()
                    .map(|fp| format!(" fp={fp}"))
                    .unwrap_or_default(),
                finding
                    .suppressed_by
                    .as_ref()
                    .map(|id| format!(" suppressed by {id}"))
                    .unwrap_or_default()
            )?;
        }
        Ok(())
    }

    pub fn write_terminal(&self, mut out: impl Write) -> io::Result<()> {
        self.write_terminal_mode(&mut out, "files")
    }

    pub fn write_terminal_staged(&self, mut out: impl Write) -> io::Result<()> {
        self.write_terminal_mode(&mut out, "staged blobs")
    }

    pub fn write_terminal_mode(&self, mut out: impl Write, label: &str) -> io::Result<()> {
        self.write_findings(&mut out)?;
        let mut reasons = BTreeMap::new();
        for skipped in &self.coverage.skipped {
            *reasons.entry(skipped.reason).or_insert(0usize) += 1;
        }
        let label = if self.coverage.scanned == 1 {
            if label == "files" {
                "file".into()
            } else if label == "staged blobs" {
                "staged blob".into()
            } else {
                label.replacen("blobs", "blob", 1)
            }
        } else {
            label.to_owned()
        };
        writeln!(out, "Scanned {} {label}", self.coverage.scanned)?;
        self.write_allow_audit(&mut out)?;
        self.write_metadata(&mut out)?;
        write!(out, "Skipped {}", self.coverage.skipped.len())?;
        let mut separator = ": ";
        for (reason, count) in &reasons {
            write!(out, "{separator}{}={count}", reason.as_str())?;
            if let Some(policy) = self.policy(*reason) {
                write!(
                    out,
                    " ({}, {})",
                    if reason.unsupported() {
                        "unsupported"
                    } else {
                        "incomplete"
                    },
                    policy.as_str()
                )?;
            } else {
                write!(out, " (informational)")?;
            }
            separator = ", ";
        }
        writeln!(out)?;
        let blocking = self
            .findings
            .iter()
            .filter(|f| f.severity == Severity::Block && f.suppressed_by.is_none())
            .count();
        if self.findings.is_empty() {
            writeln!(out, "No findings in supported categories.")?;
        } else {
            writeln!(
                out,
                "{blocking} blocking {}, {} warning {}",
                plural(blocking, "finding", "findings"),
                self.findings
                    .iter()
                    .filter(|f| f.suppressed_by.is_none())
                    .count()
                    - blocking,
                plural(
                    self.findings
                        .iter()
                        .filter(|f| f.suppressed_by.is_none())
                        .count()
                        - blocking,
                    "finding",
                    "findings"
                )
            )?;
        }
        for unsupported in [false, true] {
            let relevant: Vec<_> = reasons
                .iter()
                .filter(|(r, _)| {
                    if unsupported {
                        r.unsupported()
                    } else {
                        r.incomplete()
                    }
                })
                .collect();
            if !relevant.is_empty() {
                let policy = if unsupported {
                    self.config.policy.on_unsupported
                } else {
                    self.config.policy.on_incomplete
                };
                if policy == crate::config::PolicyAction::Warn {
                    write!(out, "WARNING: ")?;
                }
                write!(
                    out,
                    "Coverage {} (policy: {}):",
                    if unsupported {
                        "unsupported"
                    } else {
                        "incomplete"
                    },
                    policy.as_str()
                )?;
                for (reason, count) in relevant {
                    write!(out, " {}={count}", reason.as_str())?;
                }
                writeln!(out)?;
            }
        }
        if reasons.contains_key(&SkipReason::LfsPointer) {
            writeln!(out, "LFS pointer: real content is not scanned.")?;
        }
        if reasons.contains_key(&SkipReason::OcrUnavailable) {
            writeln!(
                out,
                "OCR unavailable: run cargo run --release -p xtask -- fetch-models; set --models DIR (or HUSHPUSH_MODELS) or install a release with embedded models."
            )?;
        }
        Ok(())
    }
}

pub(crate) fn plural<'a>(count: usize, singular: &'a str, plural: &'a str) -> &'a str {
    if count == 1 { singular } else { plural }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminal_controls_are_neutralized() {
        assert_eq!(sanitize_path("a\n\r\t\0\u{7f}\u{85}b"), "a??????b");
        assert_eq!(sanitize_path("a\u{1b}[31mred\u{1b}[0m.txt"), "a?red?.txt");
        assert_eq!(sanitize_path("a\u{1b}]0;title\u{7}b"), "a?b");
        assert_eq!(sanitize_path("a\u{9b}31mb"), "a?b");
    }

    #[test]
    fn terminal_preview_cannot_disclose_caller_supplied_prose() {
        let report = Report {
            findings: vec![Finding {
                fingerprint: None,
                suppressed_by: None,
                detector: "email".into(),
                detector_version: 1,
                category: Category::Email,
                severity: Severity::Warn,
                confidence: Confidence::High,
                path: "synthetic.log".into(),
                blob_oid: None,
                location: Location::Text {
                    line: 1,
                    col_start: 1,
                    col_end: 17,
                },
                preview: "synthetic-marker".into(),
                value: Sensitive::new("synthetic-marker".into()),
            }],
            coverage: Coverage::default(),
            ..Report::default()
        };
        let mut output = Vec::new();
        report.write_terminal(&mut output).unwrap();
        let output = String::from_utf8(output).unwrap();
        assert!(output.contains("[EMAIL len=16]"));
        assert!(!output.contains("synthetic-marker"));
    }
}

#[cfg(test)]
mod unicode_safety_tests {
    use super::*;
    #[test]
    fn bidi_and_invisible_controls_are_neutralized() {
        assert_eq!(sanitize_path("report\u{202e}gnp.txt"), "report?gnp.txt");
        for c in ('\u{202a}'..='\u{202e}')
            .chain('\u{2066}'..='\u{2069}')
            .chain(['\u{200b}', '\u{200c}', '\u{200d}', '\u{feff}'])
        {
            assert_eq!(sanitize_path(&format!("a{c}b")), "a?b");
        }
    }
}
