//! Sources of bytes to scan.

use std::{
    fs::{self, File},
    io::Read,
    path::Path,
};

use walkdir::WalkDir;

use crate::{
    Sensitive,
    classify::{self, ContentKind, MAX_TEXT_BYTES},
    detect::{CardDetector, Detector, EmailDetector},
    extract,
    report::{Finding, Report, SkipReason, Skipped, preview},
};

#[derive(Debug, thiserror::Error)]
#[error("scan directory is unavailable or unreadable")]
pub struct ScanError;

fn relative_path(root: &Path, path: &Path) -> String {
    let relative = path.strip_prefix(root).unwrap_or(path);
    relative
        .components()
        .map(|part| part.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

pub(super) fn skip(report: &mut Report, path: String, reason: SkipReason) {
    report.coverage.incomplete |= reason.incomplete();
    report.coverage.skipped.push(Skipped { path, reason });
}

pub fn scan_dir(root: &Path, include_reserved_domains: bool) -> Result<Report, ScanError> {
    let metadata = fs::symlink_metadata(root).map_err(|_| ScanError)?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(ScanError);
    }
    fs::read_dir(root).map_err(|_| ScanError)?;
    let mut report = Report::default();
    let entries = WalkDir::new(root)
        .follow_links(false)
        .sort_by_file_name()
        .into_iter()
        .filter_entry(|entry| {
            entry.depth() == 0 || entry.file_name() != ".git" || !entry.file_type().is_dir()
        });
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                skip(
                    &mut report,
                    relative_path(root, error.path().unwrap_or(root)),
                    SkipReason::Unreadable,
                );
                continue;
            }
        };
        let path = relative_path(root, entry.path());
        if entry.file_type().is_symlink() {
            skip(&mut report, path, SkipReason::Symlink);
            continue;
        }
        if entry.file_type().is_dir() {
            continue;
        }
        if !entry.file_type().is_file() {
            skip(&mut report, path, SkipReason::UnsupportedFileType);
            continue;
        }
        let bytes = match read_file(entry.path()) {
            Ok(bytes) => bytes,
            Err(reason) => {
                skip(&mut report, path, reason);
                continue;
            }
        };
        scan_bytes(
            &mut report,
            path,
            &bytes,
            Origin {
                blob_oid: None,
                symlink: false,
            },
            include_reserved_domains,
        );
    }
    Ok(report)
}

fn read_file(path: &Path) -> Result<Vec<u8>, SkipReason> {
    let file = File::open(path).map_err(|_| SkipReason::Unreadable)?;
    if file.metadata().map_err(|_| SkipReason::Unreadable)?.len() > MAX_TEXT_BYTES {
        return Err(SkipReason::TooLarge);
    }
    // Bound reads even when a file grows after metadata was checked.
    let mut bytes = Vec::new();
    file.take(MAX_TEXT_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| SkipReason::Unreadable)?;
    if bytes.len() as u64 > MAX_TEXT_BYTES {
        Err(SkipReason::TooLarge)
    } else {
        Ok(bytes)
    }
}

/// Provenance and mode affect interpretation, never filesystem resolution.
#[derive(Clone, Copy, Default)]
pub struct Origin<'a> {
    pub blob_oid: Option<&'a str>,
    pub symlink: bool,
}

/// Shared per-file pipeline for directory bytes and immutable Git blobs.
pub fn scan_bytes(
    report: &mut Report,
    path: String,
    bytes: &[u8],
    origin: Origin<'_>,
    include_reserved_domains: bool,
) {
    let email = EmailDetector::new(include_reserved_domains);
    let detectors: [&dyn Detector; 2] = [&email, &CardDetector];
    match if origin.symlink {
        ContentKind::Text
    } else {
        classify::classify(bytes)
    } {
        ContentKind::Image => {
            skip(report, path, SkipReason::ImageOcrNotYetSupported);
            return;
        }
        ContentKind::Binary => {
            skip(report, path, SkipReason::Binary);
            return;
        }
        ContentKind::Text => {}
    }
    let (text, partial) = extract::decode_text(bytes);
    if partial {
        skip(report, path.clone(), SkipReason::PartialDecode);
    }
    report.coverage.scanned += 1;
    let mut findings = Vec::new();
    for detector in detectors {
        for found in detector.find(&text) {
            let value = Sensitive::new(text[found.span.clone()].to_owned());
            findings.push((
                found.span.start,
                Finding {
                    detector: detector.id(),
                    detector_version: detector.version(),
                    category: detector.category(),
                    severity: detector.severity(),
                    confidence: found.confidence,
                    path: path.clone(),
                    blob_oid: origin.blob_oid.map(str::to_owned),
                    location: extract::text_location(&text, found.span),
                    preview: preview(detector.category(), value.expose().chars().count()),
                    value,
                },
            ));
        }
    }
    findings.sort_by_key(|(start, _)| *start);
    report
        .findings
        .extend(findings.into_iter().map(|(_, finding)| finding));
}

pub mod git;
