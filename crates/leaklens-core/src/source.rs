//! Sources of bytes to scan.

use std::{
    fs::{self, File},
    io::Read,
    path::Path,
};

use walkdir::WalkDir;

use crate::{
    Sensitive,
    classify::{self, ContentKind},
    extract,
    report::{Finding, Report, SkipReason, Skipped, preview_value},
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
    let mut config = crate::config::Config::default();
    config.detectors.email.include_reserved_domains = include_reserved_domains;
    scan_dir_with_config(root, &config)
}

pub fn scan_dir_with_config(
    root: &Path,
    config: &crate::config::Config,
) -> Result<Report, ScanError> {
    scan_dir_with_image(root, config, &|bytes, path, origin, _| {
        crate::ocr::default_pipeline().scan_with_config(bytes, path, origin, config)
    })
}

/// Scan using a supplied image pipeline, allowing alternate local OCR engines.
pub fn scan_dir_with_image(
    root: &Path,
    config: &crate::config::Config,
    image_scan: &impl Fn(&[u8], String, Origin<'_>, bool) -> Result<Vec<Finding>, SkipReason>,
) -> Result<Report, ScanError> {
    let include_reserved_domains = config.detectors.email.include_reserved_domains;
    let metadata = fs::symlink_metadata(root).map_err(|_| ScanError)?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(ScanError);
    }
    fs::read_dir(root).map_err(|_| ScanError)?;
    let mut report = Report::with_config(config);
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
        let bytes = match read_file_with_config(entry.path(), config) {
            Ok(bytes) => bytes,
            Err(reason) => {
                skip(&mut report, path, reason);
                continue;
            }
        };
        scan_bytes_with_image(
            &mut report,
            path,
            &bytes,
            Origin {
                blob_oid: None,
                symlink: false,
            },
            include_reserved_domains,
            image_scan,
        );
    }
    Ok(report)
}

pub(crate) fn read_file_with_config(
    path: &Path,
    config: &crate::config::Config,
) -> Result<Vec<u8>, SkipReason> {
    let mut file = File::open(path).map_err(|_| SkipReason::Unreadable)?;
    let mut prefix = Vec::new();
    file.by_ref()
        .take(8192)
        .read_to_end(&mut prefix)
        .map_err(|_| SkipReason::Unreadable)?;
    use std::io::{Seek, SeekFrom};
    file.seek(SeekFrom::Start(0))
        .map_err(|_| SkipReason::Unreadable)?;
    let image = match classify::classify(&prefix) {
        ContentKind::Binary => return Err(SkipReason::Binary),
        ContentKind::LfsPointer => return Err(SkipReason::LfsPointer),
        ContentKind::Image => true,
        ContentKind::Text => false,
    };
    let limit = if image {
        config.limits.image_bytes()
    } else {
        config.limits.text_bytes()
    };
    let reason = if image {
        SkipReason::ImageTooLarge
    } else {
        SkipReason::TooLarge
    };
    if file.metadata().map_err(|_| SkipReason::Unreadable)?.len() > limit {
        return Err(reason);
    }
    let mut bytes = Vec::new();
    file.take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| SkipReason::Unreadable)?;
    if bytes.len() as u64 > limit {
        Err(reason)
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
    let mut config = report.config.clone();
    config.detectors.email.include_reserved_domains |= include_reserved_domains;
    scan_bytes_with_image(
        report,
        path,
        bytes,
        origin,
        include_reserved_domains,
        &|bytes, path, origin, _reserved| {
            crate::ocr::default_pipeline().scan_with_config(bytes, path, origin, &config)
        },
    );
}

pub(crate) fn scan_bytes_with_image(
    report: &mut Report,
    path: String,
    bytes: &[u8],
    origin: Origin<'_>,
    include_reserved_domains: bool,
    image_scan: &impl Fn(&[u8], String, Origin<'_>, bool) -> Result<Vec<Finding>, SkipReason>,
) {
    let include_reserved_domains =
        include_reserved_domains || report.config.detectors.email.include_reserved_domains;
    let mut effective = report.config.clone();
    effective.detectors.email.include_reserved_domains = include_reserved_domains;
    let detectors = effective.detector_list();
    match if origin.symlink {
        ContentKind::Text
    } else {
        classify::classify(bytes)
    } {
        ContentKind::Image => {
            match image_scan(bytes, path.clone(), origin, include_reserved_domains) {
                Ok(mut findings) => {
                    report.config.apply_findings(&mut findings);
                    report.coverage.scanned += 1;
                    report.findings.extend(findings);
                }
                Err(reason) => skip(report, path, reason),
            }
            return;
        }
        ContentKind::Binary => {
            skip(report, path, SkipReason::Binary);
            return;
        }
        ContentKind::LfsPointer => {
            skip(report, path, SkipReason::LfsPointer);
            return;
        }
        ContentKind::Text => {}
    }
    if bytes.len() as u64 > report.config.limits.text_bytes() {
        skip(report, path, SkipReason::TooLarge);
        return;
    }
    let (text, partial) = extract::decode_text(bytes);
    if partial {
        skip(report, path.clone(), SkipReason::PartialDecode);
    }
    report.coverage.scanned += 1;
    let mut findings = Vec::new();
    for detector in detectors {
        let Some(severity) = effective.severity_for(detector.as_ref()) else {
            continue;
        };
        for found in detector.find(&text) {
            let value = Sensitive::new(text[found.span.clone()].to_owned());
            findings.push((
                found.span.start,
                Finding {
                    fingerprint: None,
                    suppressed_by: None,
                    detector: detector.id().to_owned(),
                    detector_version: detector.version(),
                    category: detector.category(),
                    severity,
                    confidence: found.confidence,
                    path: path.clone(),
                    blob_oid: origin.blob_oid.map(str::to_owned),
                    location: extract::text_location(&text, found.span),
                    preview: preview_value(detector.category(), detector.id(), value.expose()),
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
