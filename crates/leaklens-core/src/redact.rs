//! Irreversible copies, with fail-closed verification before publication.
use crate::{
    ExitCode,
    classify::{self, ContentKind},
    config::Config,
    detect::Category,
    extract,
    report::{Finding, Location, Report, SkipReason, sanitize_path},
    source::{self, Origin},
};
use image::{ImageEncoder, RgbaImage};
use std::{
    collections::BTreeMap,
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Component, Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

#[derive(Debug, thiserror::Error)]
#[error("invalid redaction source or output directory")]
pub struct RedactError;

#[derive(Default, Debug)]
pub struct RedactReport {
    pub config: Config,
    pub audit: Report,
    pub scanned: usize,
    pub written: usize,
    pub unchanged: Vec<String>,
    pub skipped: Vec<(String, &'static str)>,
}
impl RedactReport {
    pub fn exit_code(&self) -> ExitCode {
        if self
            .skipped
            .iter()
            .any(|(_, r)| *r == "redaction-incomplete")
        {
            ExitCode::BlockingFindings
        } else if !self.skipped.is_empty() {
            ExitCode::IncompleteCoverage
        } else {
            ExitCode::NoBlockingFindings
        }
    }
    pub fn write_terminal(&self, mut out: impl Write, directory: &Path) -> io::Result<()> {
        self.audit.write_findings(&mut out)?;
        for path in &self.unchanged {
            writeln!(out, "{} unchanged", sanitize_path(path))?;
        }
        let mut reasons = BTreeMap::new();
        for (path, reason) in &self.skipped {
            writeln!(out, "{} {reason}", sanitize_path(path))?;
            *reasons.entry(reason).or_insert(0) += 1;
        }
        writeln!(
            out,
            "Scanned {} {}",
            self.scanned,
            if self.scanned == 1 { "file" } else { "files" }
        )?;
        self.audit.write_allow_audit(&mut out)?;
        Report {
            config: self.config.clone(),
            ..Report::default()
        }
        .write_metadata(&mut out)?;
        write!(
            out,
            "Wrote {} redacted copies to {}; {} unchanged; {} not redacted",
            self.written,
            sanitize_path(&directory.to_string_lossy()),
            self.unchanged.len(),
            self.skipped.len()
        )?;
        if !reasons.is_empty() {
            write!(out, " (")?;
        }
        for (i, (reason, count)) in reasons.iter().enumerate() {
            if i > 0 {
                write!(out, ", ")?;
            }
            write!(out, "{reason}={count}")?;
        }
        if !reasons.is_empty() {
            write!(out, ")")?;
        }
        writeln!(out)?;
        if !self.skipped.is_empty() {
            writeln!(out, "Coverage incomplete")?;
        }
        Ok(())
    }
}

// Validate both Windows and Unix spellings, regardless of the host OS.
fn safe_name(path: &str) -> bool {
    !path.is_empty()
        && !path.starts_with(['/', '\\'])
        && path.split(['/', '\\']).all(|part| {
            let stem = part
                .split('.')
                .next()
                .unwrap_or("")
                .trim_end()
                .to_ascii_uppercase();
            !part.is_empty()
                && part != "."
                && part != ".."
                && !part.ends_with(['.', ' '])
                && !part
                    .chars()
                    .any(|c| c.is_control() || "<>:\"|?*".contains(c))
                && !matches!(
                    stem.as_str(),
                    "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
                )
                && !(stem.len() == 4
                    && (stem.starts_with("COM") || stem.starts_with("LPT"))
                    && matches!(stem.as_bytes()[3], b'1'..=b'9'))
                && !["COM¹", "COM²", "COM³", "LPT¹", "LPT²", "LPT³"].contains(&stem.as_str())
        })
}
fn resolved(path: &Path) -> Result<PathBuf, RedactError> {
    if path.components().any(|c| matches!(c, Component::ParentDir)) {
        return Err(RedactError);
    }
    if path.exists() {
        return fs::canonicalize(path).map_err(|_| RedactError);
    }
    let name = path.file_name().ok_or(RedactError)?;
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    Ok(resolved(parent)?.join(name))
}
fn layout(path: &str, image: bool) -> String {
    if image && !path.ends_with(".png") {
        format!("{path}.png")
    } else {
        path.to_owned()
    }
}

pub fn redact(path: &Path, out: &Path, reserved: bool) -> Result<RedactReport, RedactError> {
    let mut config = Config::default();
    config.detectors.email.include_reserved_domains = reserved;
    redact_with_config(path, out, &config)
}
pub fn redact_with_config(
    path: &Path,
    out: &Path,
    config: &Config,
) -> Result<RedactReport, RedactError> {
    redact_config_with(path, out, config, &|bytes, path, origin, _| {
        crate::ocr::default_pipeline().scan_with_config(bytes, path, origin, config)
    })
}
#[cfg(test)]
fn redact_with(
    path: &Path,
    out: &Path,
    reserved: bool,
    image_scan: &impl Fn(&[u8], String, Origin<'_>, bool) -> Result<Vec<Finding>, SkipReason>,
) -> Result<RedactReport, RedactError> {
    let mut config = Config::default();
    config.detectors.email.include_reserved_domains = reserved;
    redact_config_with(path, out, &config, image_scan)
}
fn redact_config_with(
    path: &Path,
    out: &Path,
    config: &Config,
    image_scan: &impl Fn(&[u8], String, Origin<'_>, bool) -> Result<Vec<Finding>, SkipReason>,
) -> Result<RedactReport, RedactError> {
    let reserved = config.detectors.email.include_reserved_domains;
    let meta = fs::symlink_metadata(path).map_err(|_| RedactError)?;
    if meta.file_type().is_symlink() || !(meta.is_file() || meta.is_dir()) {
        return Err(RedactError);
    }
    let root = fs::canonicalize(path).map_err(|_| RedactError)?;
    let target = resolved(out)?;
    if root.starts_with(&target) || target.starts_with(&root) {
        return Err(RedactError);
    }
    if target.exists()
        && (!target.is_dir()
            || fs::read_dir(&target)
                .map_err(|_| RedactError)?
                .next()
                .is_some())
    {
        return Err(RedactError);
    }
    let base = if meta.is_dir() {
        root.as_path()
    } else {
        root.parent().ok_or(RedactError)?
    };
    let mut result = RedactReport {
        config: config.clone(),
        audit: Report::with_config(config),
        ..RedactReport::default()
    };
    let mut files = Vec::new();
    for entry in walkdir::WalkDir::new(&root)
        .follow_links(false)
        .sort_by_file_name()
        .into_iter()
        .filter_entry(|e| e.depth() == 0 || e.file_name() != ".git" || !e.file_type().is_dir())
    {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => {
                result
                    .skipped
                    .push(("<unreadable-entry>".into(), "unreadable"));
                continue;
            }
        };
        if entry.file_type().is_dir() {
            continue;
        }
        let relative = entry
            .path()
            .strip_prefix(base)
            .map_err(|_| RedactError)?
            .to_str()
            .map(str::to_owned);
        let Some(relative) = relative else {
            result
                .skipped
                .push(("<non-unicode-name>".into(), "unsafe-output-name"));
            continue;
        };
        let relative = relative.replace('\\', "/");
        if !safe_name(&relative) {
            result.skipped.push((relative, "unsafe-output-name"));
            continue;
        }
        if !entry.file_type().is_file() {
            result.skipped.push((relative, "unsupported-file-type"));
            continue;
        }
        let bytes = match source::read_file_with_config(entry.path(), config) {
            Ok(b) => b,
            Err(r) => {
                result.skipped.push((relative, r.as_str()));
                continue;
            }
        };
        let image = classify::classify(&bytes) == ContentKind::Image;
        // Store paths only: don't retain the entire input tree in memory.
        files.push((
            entry.path().to_owned(),
            relative.clone(),
            layout(&relative, image),
        ));
    }
    // Case-fold also on Unix, so a result is portable to Windows.
    let mut counts = BTreeMap::new();
    for (_, _, name) in &files {
        *counts.entry(name.to_lowercase()).or_insert(0) += 1;
    }
    fs::create_dir_all(&target).map_err(|_| RedactError)?;
    for (input, relative, name) in files {
        if counts[&name.to_lowercase()] > 1 || files_conflict(&name, &counts) {
            result.skipped.push((relative, "output-path-collision"));
            continue;
        }
        let mut run = || -> Result<bool, &'static str> {
            let bytes =
                source::read_file_with_config(&input, config).map_err(SkipReason::as_str)?;
            let mut report = Report {
                config: config.clone(),
                ..Report::default()
            };
            result.scanned += 1;
            source::scan_bytes_with_image(
                &mut report,
                relative.clone(),
                &bytes,
                Origin::default(),
                reserved,
                image_scan,
            );
            result
                .audit
                .findings
                .extend(report.findings.iter().cloned());
            if report
                .coverage
                .skipped
                .iter()
                .any(|s| s.reason == SkipReason::PartialDecode)
            {
                return Err("redact-unsafe-lossy");
            }
            if let Some(s) = report.coverage.skipped.first() {
                return Err(s.reason.as_str());
            }
            if report.findings.is_empty() {
                return Ok(false);
            }
            let output = if classify::classify(&bytes) == ContentKind::Image {
                let mut image = crate::ocr::decode_rgba_with_limits(&bytes, &config.limits)
                    .map_err(SkipReason::as_str)?;
                fill(&mut image, &report.findings)?;
                encode(&image)?
            } else {
                redact_text(&bytes, &report.findings)?
            };
            let destination = target.join(&name);
            let parent = destination.parent().ok_or("unsafe-output-name")?;
            fs::create_dir_all(parent).map_err(|_| "output-write-failed")?;
            if !fs::canonicalize(parent)
                .map_err(|_| "output-write-failed")?
                .starts_with(&target)
            {
                return Err("unsafe-output-name");
            }
            atomic_verified(&destination, &output, |saved| {
                let mut verification = Report {
                    config: config.clone(),
                    ..Report::default()
                };
                source::scan_bytes_with_image(
                    &mut verification,
                    relative.clone(),
                    saved,
                    Origin::default(),
                    reserved,
                    image_scan,
                );
                if !verification.findings.is_empty() {
                    Err("redaction-incomplete")
                } else if !verification.coverage.skipped.is_empty() {
                    Err("redact-rescan-failed")
                } else {
                    Ok(())
                }
            })?;
            Ok(true)
        };
        match run() {
            Ok(true) => result.written += 1,
            Ok(false) => result.unchanged.push(relative),
            Err(reason) => result.skipped.push((relative, reason)),
        }
    }
    Ok(result)
}
fn files_conflict(name: &str, counts: &BTreeMap<String, usize>) -> bool {
    let name = name.to_lowercase();
    counts.keys().any(|other| {
        other.starts_with(&(name.clone() + "/")) || name.starts_with(&(other.clone() + "/"))
    })
}
fn offset(text: &str, line: usize, column: usize) -> Option<usize> {
    if line == 0 || column == 0 {
        return None;
    }
    let start = if line == 1 {
        0
    } else {
        text.match_indices('\n').nth(line - 2)?.0 + 1
    };
    let rest = &text[start..];
    let index = rest
        .char_indices()
        .nth(column - 1)
        .map(|(i, _)| i)
        .or_else(|| (rest.chars().count() == column - 1).then_some(rest.len()))?;
    Some(start + index)
}
fn redact_text(bytes: &[u8], findings: &[Finding]) -> Result<Vec<u8>, &'static str> {
    let (text, lossy) = extract::decode_text(bytes);
    if lossy {
        return Err("redact-unsafe-lossy");
    }
    let mut spans = Vec::new();
    for f in findings {
        let Location::Text {
            line,
            col_start,
            col_end,
        } = f.location
        else {
            return Err("redact-invalid-location");
        };
        let start = offset(&text, line, col_start).ok_or("redact-invalid-location")?;
        let end = offset(&text, line, col_end).ok_or("redact-invalid-location")?;
        if start >= end {
            return Err("redact-invalid-location");
        }
        spans.push((start, end, f.category));
    }
    spans.sort_by_key(|s| s.0);
    let mut merged: Vec<(usize, usize, Category)> = Vec::new();
    for (start, end, category) in spans {
        if let Some(last) = merged.last_mut()
            && start <= last.1
        {
            last.1 = last.1.max(end);
            if category == Category::PaymentCard {
                last.2 = category;
            }
        } else {
            merged.push((start, end, category));
        }
    }
    let mut output = Vec::new();
    if bytes.starts_with(b"\xef\xbb\xbf") {
        output.extend_from_slice(b"\xef\xbb\xbf");
    }
    let mut cursor = 0;
    for (start, end, category) in merged {
        output.extend_from_slice(&text.as_bytes()[cursor..start]);
        output.extend_from_slice(match category {
            Category::Email => b"[REDACTED:EMAIL]",
            Category::PaymentCard => b"[REDACTED:CARD]",
        });
        cursor = end;
    }
    output.extend_from_slice(&text.as_bytes()[cursor..]);
    Ok(output)
}
fn fill(image: &mut RgbaImage, findings: &[Finding]) -> Result<(), &'static str> {
    for f in findings {
        let Location::Bbox { x, y, w, h } = f.location else {
            return Err("redact-invalid-location");
        };
        for y in y.min(image.height())..y.saturating_add(h).min(image.height()) {
            for x in x.min(image.width())..x.saturating_add(w).min(image.width()) {
                image.put_pixel(x, y, image::Rgba([0, 0, 0, 255]));
            }
        }
    }
    Ok(())
}
fn encode(image: &RgbaImage) -> Result<Vec<u8>, &'static str> {
    let mut output = Vec::new();
    image::codecs::png::PngEncoder::new(&mut output)
        .write_image(
            image.as_raw(),
            image.width(),
            image.height(),
            image::ExtendedColorType::Rgba8,
        )
        .map_err(|_| "output-encode-failed")?;
    Ok(output)
}
struct Temp(PathBuf);
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}
static NEXT: AtomicU64 = AtomicU64::new(0);
fn atomic_verified(
    destination: &Path,
    bytes: &[u8],
    verify: impl FnOnce(&[u8]) -> Result<(), &'static str>,
) -> Result<(), &'static str> {
    let parent = destination.parent().ok_or("unsafe-output-name")?;
    let (temp, mut file) = (0..100)
        .find_map(|_| {
            let path = parent.join(format!(
                ".leaklens-{}-{}.tmp",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
                .ok()
                .map(|f| (Temp(path), f))
        })
        .ok_or("output-write-failed")?;
    file.write_all(bytes)
        .and_then(|_| file.sync_all())
        .map_err(|_| "output-write-failed")?;
    drop(file);
    let saved = fs::read(&temp.0).map_err(|_| "output-write-failed")?;
    // Verify the bytes actually written; a failed output never receives its final name.
    verify(&saved)?;
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        #[link(name = "kernel32")]
        unsafe extern "system" {
            fn MoveFileW(existing: *const u16, new: *const u16) -> i32;
        }
        let from: Vec<u16> = temp.0.as_os_str().encode_wide().chain(Some(0)).collect();
        let to: Vec<u16> = destination
            .as_os_str()
            .encode_wide()
            .chain(Some(0))
            .collect();
        // Unlike std::fs::rename, MoveFileW never replaces an existing destination.
        if unsafe { MoveFileW(from.as_ptr(), to.as_ptr()) } == 0 {
            return Err("output-write-failed");
        }
    }
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        use std::{ffi::CString, os::unix::ffi::OsStrExt};
        let from = CString::new(temp.0.as_os_str().as_bytes()).map_err(|_| "unsafe-output-name")?;
        let to =
            CString::new(destination.as_os_str().as_bytes()).map_err(|_| "unsafe-output-name")?;
        #[cfg(target_os = "linux")]
        unsafe extern "C" {
            fn renameat2(
                old_dir: i32,
                old: *const std::ffi::c_char,
                new_dir: i32,
                new: *const std::ffi::c_char,
                flags: u32,
            ) -> i32;
        }
        #[cfg(target_os = "macos")]
        unsafe extern "C" {
            fn renamex_np(
                old: *const std::ffi::c_char,
                new: *const std::ffi::c_char,
                flags: u32,
            ) -> i32;
        }
        // RENAME_NOREPLACE / RENAME_EXCL preserve no-clobber semantics atomically.
        // Both C strings are alive and NUL-terminated for the duration of the call.
        #[cfg(target_os = "linux")]
        let status = unsafe { renameat2(-100, from.as_ptr(), -100, to.as_ptr(), 1) };
        #[cfg(target_os = "macos")]
        let status = unsafe { renamex_np(from.as_ptr(), to.as_ptr(), 4) };
        if status != 0 {
            return Err("output-write-failed");
        }
    }
    #[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
    return Err("output-write-failed");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        Sensitive,
        detect::{Confidence, Severity},
        ocr::{OcrChar, OcrEngine, OcrError, OcrLine},
    };
    struct TestDir(PathBuf);
    impl TestDir {
        fn new() -> Self {
            let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
                "synthetic-t5-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for TestDir {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }
    fn finding(location: Location) -> Finding {
        Finding {
            fingerprint: None,
            suppressed_by: None,
            detector: "email",
            detector_version: 1,
            category: Category::Email,
            severity: Severity::Warn,
            confidence: Confidence::High,
            path: "synthetic".into(),
            blob_oid: None,
            location,
            preview: String::new(),
            value: Sensitive::new(String::new()),
        }
    }
    #[test]
    fn empty_skipped_footer_has_no_parentheses() {
        let mut output = Vec::new();
        RedactReport::default()
            .write_terminal(&mut output, Path::new("synthetic-output"))
            .unwrap();
        let output = String::from_utf8(output).unwrap();
        assert!(output.contains("0 not redacted\n"));
        assert!(!output.contains("()"));
    }
    #[test]
    fn redaction_respects_disabled_detector_and_keeps_failure_exit_rules() {
        use crate::config::{DetectorSeverity, PolicyAction};
        let dir = TestDir::new();
        let source = dir.0.join("input");
        fs::create_dir(&source).unwrap();
        fs::write(source.join("synthetic.log"), b"demo@example.com").unwrap();
        let mut config = Config::default();
        config.detectors.email.include_reserved_domains = true;
        config.detectors.email.severity = DetectorSeverity::Off;
        config.policy.on_incomplete = PolicyAction::Warn;
        let report = redact_with_config(&source, &dir.0.join("out"), &config).unwrap();
        assert_eq!(report.written, 0);
        assert_eq!(report.scanned, 1);
        assert_eq!(report.unchanged, vec!["synthetic.log"]);
        assert_eq!(report.exit_code(), ExitCode::NoBlockingFindings);
        let mut output = Vec::new();
        report
            .write_terminal(&mut output, &dir.0.join("out"))
            .unwrap();
        let output = String::from_utf8(output).unwrap();
        assert!(output.contains("Scanned 1 file"));
        assert!(output.contains("Detectors: card@2 (block)"));
        assert!(!output.contains("email@"));
        assert!(output.contains("Config: defaults"));
        fs::write(source.join("binary"), b"\0\0\0").unwrap();
        let report = redact_with_config(&source, &dir.0.join("out-two"), &config).unwrap();
        assert_eq!(report.exit_code(), ExitCode::IncompleteCoverage);
        let mut output = Vec::new();
        report
            .write_terminal(&mut output, &dir.0.join("out-two"))
            .unwrap();
        assert!(
            !String::from_utf8(output)
                .unwrap()
                .contains("demo@example.com")
        );
    }
    #[test]
    fn allow_audit_never_exempts_a_value_from_redaction() {
        let dir = TestDir::new();
        let source = dir.0.join("input");
        fs::create_dir(&source).unwrap();
        fs::write(source.join("synthetic.log"), b"demo@example.com").unwrap();
        let config = Config::parse(
            "[detectors.email]\ninclude_reserved_domains = true\n[[allow]]\nid = 'fixture'\npaths = ['synthetic.log']\nreason = 'Synthetic fixture'\n",
        ).unwrap();
        let out = dir.0.join("out");
        let mut report = redact_with_config(&source, &out, &config).unwrap();
        report.audit.apply_allowlists(None, false);
        assert_eq!(report.written, 1);
        assert_eq!(report.exit_code(), ExitCode::NoBlockingFindings);
        assert_eq!(
            fs::read(out.join("synthetic.log")).unwrap(),
            b"[REDACTED:EMAIL]"
        );
        let mut terminal = Vec::new();
        report.write_terminal(&mut terminal, &out).unwrap();
        let terminal = String::from_utf8(terminal).unwrap();
        assert!(terminal.contains("suppressed by allow:fixture"));
        assert!(!terminal.contains("demo@example.com"));
    }
    #[test]
    fn bom_crlf_unicode_overlap_adjacency_and_trailing_newline_are_byte_exact() {
        let input = b"\xef\xbb\xbf\xc3\xa9 header\r\nabcdefghij tail\r\n";
        let f = |a, b| {
            finding(Location::Text {
                line: 2,
                col_start: a,
                col_end: b,
            })
        };
        let output = redact_text(input, &[f(1, 5), f(3, 7), f(7, 11)]).unwrap();
        assert_eq!(
            output,
            b"\xef\xbb\xbf\xc3\xa9 header\r\n[REDACTED:EMAIL] tail\r\n"
        );
        assert_eq!(redact_text(b"x\xff", &[]), Err("redact-unsafe-lossy"));
    }
    fn chunks(bytes: &[u8]) -> Vec<&[u8]> {
        let mut offset = 8;
        let mut list = vec![];
        while offset < bytes.len() {
            let len = u32::from_be_bytes(bytes[offset..offset + 4].try_into().unwrap()) as usize;
            list.push(&bytes[offset + 4..offset + 8]);
            offset += len + 12;
        }
        assert_eq!(offset, bytes.len());
        list
    }
    #[test]
    fn opaque_fill_preserves_all_other_rgba_bytes_and_png_has_only_required_chunks() {
        let mut image = RgbaImage::from_fn(12, 10, |x, y| {
            image::Rgba([x as u8, y as u8, 93, (x * y) as u8])
        });
        let original = image.clone();
        fill(
            &mut image,
            &[
                finding(Location::Bbox {
                    x: 2,
                    y: 3,
                    w: 4,
                    h: 5,
                }),
                finding(Location::Bbox {
                    x: 10,
                    y: 8,
                    w: u32::MAX,
                    h: u32::MAX,
                }),
            ],
        )
        .unwrap();
        for (x, y, p) in image.enumerate_pixels() {
            if ((2..6).contains(&x) && (3..8).contains(&y)) || (x >= 10 && y >= 8) {
                assert_eq!(p.0, [0, 0, 0, 255]);
            } else {
                assert_eq!(p, original.get_pixel(x, y));
            }
        }
        let output = encode(&image).unwrap();
        assert_eq!(chunks(&output), vec![b"IHDR".as_slice(), b"IDAT", b"IEND"]);
        assert_eq!(crate::ocr::decode_rgba(&output).unwrap(), image);
    }
    #[test]
    fn unsafe_names_and_image_layout() {
        for name in [
            "CON",
            "nul.txt",
            "con .txt",
            "COM1.log",
            "LPT9.PNG",
            "a/aux.ext",
            "../x",
            "/x",
            "C:\\x",
            "a/../b",
            "a\\..\\b",
            "x.",
            "COM¹.txt",
        ] {
            assert!(!safe_name(name));
        }
        assert!(safe_name("a/valid.log"));
        assert_eq!(layout("shot.jpg", true), "shot.jpg.png");
        assert_eq!(layout("shot.png", true), "shot.png");
    }
    struct Fake {
        residual: bool,
    }
    impl OcrEngine for Fake {
        fn recognize(&self, img: &image::RgbImage) -> Result<Vec<OcrLine>, OcrError> {
            if !self.residual && img.get_pixel(0, 0).0 == [0, 0, 0] {
                return Ok(vec![]);
            }
            let text = "demo@example.com";
            Ok(vec![OcrLine {
                text: text.into(),
                chars: text
                    .chars()
                    .map(|ch| OcrChar {
                        ch,
                        bbox: [0., 0., 8., 8.],
                    })
                    .collect(),
            }])
        }
    }
    fn fake_scan(
        engine: &dyn OcrEngine,
        bytes: &[u8],
        path: String,
        origin: Origin<'_>,
        reserved: bool,
    ) -> Result<Vec<Finding>, SkipReason> {
        let image = crate::ocr::decode(bytes)?;
        let dimensions = image.dimensions();
        let lines = engine
            .recognize(&image)
            .map_err(|_| SkipReason::OcrUnavailable)?;
        crate::ocr::findings(&lines, dimensions, path, origin, reserved)
    }
    fn png_chunk(kind: &[u8], payload: &[u8]) -> Vec<u8> {
        let mut output = (payload.len() as u32).to_be_bytes().to_vec();
        output.extend_from_slice(kind);
        output.extend_from_slice(payload);
        let mut crc = !0u32;
        for byte in &output[4..] {
            crc ^= u32::from(*byte);
            for _ in 0..8 {
                crc = (crc >> 1) ^ (0xedb88320u32.wrapping_mul(crc & 1));
            }
        }
        output.extend_from_slice(&(!crc).to_be_bytes());
        output
    }
    #[test]
    fn png_text_and_jpeg_exif_cannot_survive_redaction() {
        let dir = TestDir::new();
        let source = dir.0.join("input");
        fs::create_dir(&source).unwrap();
        let image = RgbaImage::from_pixel(32, 32, image::Rgba([255, 255, 255, 31]));
        let mut png = encode(&image).unwrap();
        png.splice(33..33, png_chunk(b"tEXt", b"Comment\0demo@example.com"));
        let mut jpeg = vec![];
        image::codecs::jpeg::JpegEncoder::new(&mut jpeg)
            .encode_image(&image::DynamicImage::ImageRgba8(image))
            .unwrap();
        let payload = b"Exif\0\0II\x2a\0\x08\0\0\0\0\0\0\0\0\0demo@example.com";
        let mut app1 = vec![0xff, 0xe1];
        app1.extend_from_slice(&((payload.len() + 2) as u16).to_be_bytes());
        app1.extend_from_slice(payload);
        jpeg.splice(2..2, app1);
        assert!(png.windows(16).any(|w| w == b"demo@example.com"));
        assert!(jpeg.windows(16).any(|w| w == b"demo@example.com"));
        fs::write(source.join("a.png"), &png).unwrap();
        fs::write(source.join("b.jpg"), &jpeg).unwrap();
        let out = dir.0.join("out");
        let report = redact_with(&source, &out, true, &|b, p, o, r| {
            fake_scan(&Fake { residual: false }, b, p, o, r)
        })
        .unwrap();
        assert_eq!(report.written, 2);
        assert_eq!(report.scanned, 2);
        assert_eq!(report.exit_code(), ExitCode::NoBlockingFindings);
        for name in ["a.png", "b.jpg.png"] {
            let bytes = fs::read(out.join(name)).unwrap();
            assert!(!bytes.windows(16).any(|w| w == b"demo@example.com"));
            assert_eq!(chunks(&bytes), vec![b"IHDR".as_slice(), b"IDAT", b"IEND"]);
        }
        assert_eq!(fs::read(source.join("a.png")).unwrap(), png);
        assert_eq!(fs::read(source.join("b.jpg")).unwrap(), jpeg);
    }
    #[test]
    fn residuals_and_failed_rescans_leave_no_output_or_temp_and_have_exit_precedence() {
        let dir = TestDir::new();
        let source = dir.0.join("input");
        fs::create_dir(&source).unwrap();
        fs::write(
            source.join("a.png"),
            encode(&RgbaImage::from_pixel(32, 32, image::Rgba([255; 4]))).unwrap(),
        )
        .unwrap();
        fs::write(source.join("lossy"), b"demo@example.com\xff").unwrap();
        let out = dir.0.join("out");
        let report = redact_with(&source, &out, true, &|b, p, o, r| {
            fake_scan(&Fake { residual: true }, b, p, o, r)
        })
        .unwrap();
        assert_eq!(report.exit_code(), ExitCode::BlockingFindings);
        assert!(!report.audit.findings.is_empty());
        let mut terminal = Vec::new();
        report.write_terminal(&mut terminal, &out).unwrap();
        let terminal = String::from_utf8(terminal).unwrap();
        assert!(terminal.contains("a.png redaction-incomplete"));
        assert!(terminal.contains("Coverage incomplete"));
        assert!(!terminal.contains("demo@example.com"));
        assert_eq!(report.written, 0);
        assert_eq!(fs::read_dir(&out).unwrap().count(), 0);
        assert!(
            report
                .skipped
                .iter()
                .any(|(_, r)| *r == "redact-unsafe-lossy")
        );
        assert!(
            report
                .skipped
                .iter()
                .any(|(_, r)| *r == "redaction-incomplete")
        );
        fs::remove_file(source.join("lossy")).unwrap();
        let calls = std::cell::Cell::new(0);
        let report = redact_with(&source, &out, true, &|b, p, o, r| {
            calls.set(calls.get() + 1);
            if calls.get() == 2 {
                Err(SkipReason::OcrTimeout)
            } else {
                fake_scan(&Fake { residual: false }, b, p, o, r)
            }
        })
        .unwrap();
        assert_eq!(report.exit_code(), ExitCode::IncompleteCoverage);
        assert_eq!(fs::read_dir(&out).unwrap().count(), 0);
    }
    #[test]
    fn existing_destination_is_never_replaced_and_temp_is_removed() {
        let dir = TestDir::new();
        let dest = dir.0.join("existing");
        fs::write(&dest, b"original").unwrap();
        assert_eq!(
            atomic_verified(&dest, b"replacement", |_| Ok(())),
            Err("output-write-failed")
        );
        assert_eq!(fs::read(&dest).unwrap(), b"original");
        assert_eq!(fs::read_dir(&dir.0).unwrap().count(), 1);
        assert_eq!(
            atomic_verified(&dir.0.join("new"), b"replacement", |_| Err(
                "redaction-incomplete"
            )),
            Err("redaction-incomplete")
        );
        assert_eq!(fs::read_dir(&dir.0).unwrap().count(), 1);
    }
    #[test]
    fn reserved_filename_is_skipped_as_incomplete_without_a_copy() {
        let dir = TestDir::new();
        let source = dir.0.join("input");
        fs::create_dir(&source).unwrap();
        // The extended Windows path lets the test create a literal reserved filename.
        let source = fs::canonicalize(source).unwrap();
        fs::write(source.join("CON.txt"), b"demo@example.com").unwrap();
        let out = dir.0.join("out");
        let report = redact(&source, &out, true).unwrap();
        assert_eq!(report.exit_code(), ExitCode::IncompleteCoverage);
        assert_eq!(
            report.skipped,
            vec![("CON.txt".into(), "unsafe-output-name")]
        );
        assert_eq!(report.written, 0);
        assert_eq!(fs::read_dir(out).unwrap().count(), 0);
    }

    #[test]
    fn output_collisions_skip_both_files() {
        let dir = TestDir::new();
        let source = dir.0.join("input");
        fs::create_dir(&source).unwrap();
        fs::write(
            source.join("shot.jpg"),
            encode(&RgbaImage::from_pixel(32, 32, image::Rgba([255; 4]))).unwrap(),
        )
        .unwrap();
        fs::write(
            source.join("shot.jpg.png"),
            encode(&RgbaImage::from_pixel(32, 32, image::Rgba([255; 4]))).unwrap(),
        )
        .unwrap();
        let report = redact_with(&source, &dir.0.join("out"), true, &|b, p, o, r| {
            fake_scan(&Fake { residual: false }, b, p, o, r)
        })
        .unwrap();
        assert_eq!(report.skipped.len(), 2);
        assert_eq!(report.written, 0);
        assert!(
            report
                .skipped
                .iter()
                .all(|(_, r)| *r == "output-path-collision")
        );
    }
}
