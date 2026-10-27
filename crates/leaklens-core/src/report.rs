//! Masked findings and coverage reports.

use std::{
    collections::BTreeMap,
    io::{self, Write},
};

use crate::{
    ExitCode, Sensitive,
    detect::{Category, Confidence, Severity},
};

#[derive(Debug)]
pub struct Finding {
    pub detector: &'static str,
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
        }
    }

    pub fn incomplete(self) -> bool {
        !matches!(self, Self::Binary | Self::Symlink | Self::Submodule)
    }
}

#[derive(Default, Debug)]
pub struct Report {
    pub findings: Vec<Finding>,
    pub coverage: Coverage,
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
                Some(next) if !next.is_control() => {
                    result.push(next);
                }
                Some(_) => result.push('?'),
                None => {}
            }
        } else {
            result.push(if c.is_control() { '?' } else { c });
        }
    }
    result
}

pub fn preview(category: Category, char_count: usize) -> String {
    let name = match category {
        Category::Email => "EMAIL",
        Category::PaymentCard => "CARD",
    };
    format!("[{name} len={char_count}]")
}

impl Report {
    pub fn exit_code(&self) -> ExitCode {
        if self
            .findings
            .iter()
            .any(|finding| finding.severity == Severity::Block)
        {
            ExitCode::BlockingFindings
        } else if self.coverage.incomplete {
            ExitCode::IncompleteCoverage
        } else {
            ExitCode::NoBlockingFindings
        }
    }

    pub fn write_terminal(&self, mut out: impl Write) -> io::Result<()> {
        self.write_terminal_mode(&mut out, false)
    }

    pub fn write_terminal_staged(&self, mut out: impl Write) -> io::Result<()> {
        self.write_terminal_mode(&mut out, true)
    }

    fn write_terminal_mode(&self, mut out: impl Write, staged: bool) -> io::Result<()> {
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
                "{}{location}  {:?}  {:?}  {:?}  {}",
                sanitize_path(&finding.path),
                finding.severity,
                finding.category,
                finding.confidence,
                preview(finding.category, finding.value.expose().chars().count())
            )?;
        }
        let mut reasons = BTreeMap::new();
        for skipped in &self.coverage.skipped {
            *reasons.entry(skipped.reason).or_insert(0usize) += 1;
        }
        if staged {
            writeln!(out, "Scanned {} staged blobs", self.coverage.scanned)?;
        } else {
            writeln!(out, "Scanned: {}", self.coverage.scanned)?;
        }
        write!(out, "Skipped: {}", self.coverage.skipped.len())?;
        for (reason, count) in &reasons {
            write!(out, "  {}={count}", reason.as_str())?;
        }
        writeln!(out)?;
        let blocking = self
            .findings
            .iter()
            .filter(|f| f.severity == Severity::Block)
            .count();
        if self.findings.is_empty() {
            writeln!(out, "No findings in supported categories.")?;
        } else {
            writeln!(
                out,
                "{blocking} blocking, {} warning findings",
                self.findings.len() - blocking
            )?;
        }
        if self.coverage.incomplete {
            write!(out, "Coverage incomplete:")?;
            for (reason, count) in reasons.iter().filter(|(reason, _)| reason.incomplete()) {
                write!(out, " {}={count}", reason.as_str())?;
            }
            writeln!(out)?;
        }
        if reasons.contains_key(&SkipReason::OcrUnavailable) {
            writeln!(
                out,
                "OCR unavailable: run cargo run --release -p xtask -- fetch-models; set --models DIR (or LEAKLENS_MODELS) and ORT_DYLIB_PATH to the local CPU runtime."
            )?;
        }
        Ok(())
    }
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
                detector: "email",
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
        };
        let mut output = Vec::new();
        report.write_terminal(&mut output).unwrap();
        let output = String::from_utf8(output).unwrap();
        assert!(output.contains("[EMAIL len=16]"));
        assert!(!output.contains("synthetic-marker"));
    }
}
