//! Bounded image decoding and local OCR. Scans never acquire models or runtimes.
use crate::{
    Sensitive,
    config::{Config, Limits},
    detect::Confidence,
    report::{Finding, Location, SkipReason, preview_value},
    source::Origin,
};
use image::{ImageReader, RgbImage};
use std::{
    io::Cursor,
    ops::Range,
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock, mpsc},
    time::Duration,
};
use unicode_normalization::UnicodeNormalization;
mod ppocr;
pub use ppocr::PpOcrEngine;
pub const MAX_IMAGE_BYTES: u64 = 50 * 1024 * 1024;
pub const MAX_PIXELS: u64 = 50_000_000;
pub const MAX_ALLOC: u64 = 512 * 1024 * 1024;
#[derive(thiserror::Error, Debug)]
#[error("OCR unavailable or inference failed")]
pub struct OcrError;
impl From<ort::Error> for OcrError {
    fn from(_: ort::Error) -> Self {
        Self
    }
}

pub struct OcrChar {
    pub ch: char,
    /// [left, top, right, bottom] in displayed (EXIF-oriented) image pixels.
    pub bbox: [f32; 4],
}
pub struct OcrLine {
    pub text: String,
    pub chars: Vec<OcrChar>,
}
impl std::fmt::Debug for OcrLine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("OcrLine([redacted])")
    }
}
impl std::fmt::Debug for OcrChar {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("OcrChar([redacted])")
    }
}
pub trait OcrEngine: Send + Sync {
    fn recognize(&self, img: &RgbImage) -> Result<Vec<OcrLine>, OcrError>;
}
pub fn normalize_char(ch: char) -> char {
    let source = ch.to_string();
    let mut chars = source.nfkc();
    let first = chars.next().unwrap_or(ch);
    if chars.next().is_none() { first } else { ch }
}
pub fn runtime_filename() -> &'static str {
    if cfg!(windows) {
        "onnxruntime.dll"
    } else if cfg!(target_os = "macos") {
        "libonnxruntime.dylib"
    } else {
        "libonnxruntime.so"
    }
}
pub fn resolve_models(explicit: Option<PathBuf>) -> Option<PathBuf> {
    explicit
        .or_else(|| std::env::var_os("HUSHPUSH_MODELS").map(PathBuf::from))
        .or_else(|| {
            std::env::current_exe()
                .ok()
                .and_then(|p| p.parent().map(|p| p.join("models")))
                .filter(|p| p.is_dir())
        })
        .or_else(|| {
            if cfg!(debug_assertions) {
                Some(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../models"))
            } else {
                None
            }
        })
}
static MODELS: OnceLock<PathBuf> = OnceLock::new();
static DEFAULT_PIPELINE: OnceLock<ImagePipeline> = OnceLock::new();
/// Configure once, before the first scan. Does not modify process environment.
pub fn configure_models(path: PathBuf) -> Result<(), PathBuf> {
    MODELS.set(path)
}
pub fn default_pipeline() -> &'static ImagePipeline {
    DEFAULT_PIPELINE.get_or_init(|| ImagePipeline::new(MODELS.get().cloned(), true))
}
pub fn decode(bytes: &[u8]) -> Result<RgbImage, SkipReason> {
    decode_rgba(bytes).map(|i| image::DynamicImage::ImageRgba8(i).into_rgb8())
}
pub fn decode_rgba(bytes: &[u8]) -> Result<image::RgbaImage, SkipReason> {
    decode_rgba_with_limits(bytes, &Limits::default())
}
pub fn decode_rgba_with_limits(
    bytes: &[u8],
    configured: &Limits,
) -> Result<image::RgbaImage, SkipReason> {
    if bytes.len() as u64 > configured.image_bytes() {
        return Err(SkipReason::ImageTooLarge);
    }
    // Inspect dimensions even when a truncated stream cannot construct a decoder.
    if let Some((w, h)) = header_dimensions(bytes)
        && u64::from(w) * u64::from(h) > configured.pixels()
    {
        return Err(SkipReason::ImageTooLarge);
    }
    let mut reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|_| SkipReason::ImageDecodeFailed)?;
    if !matches!(
        reader.format(),
        Some(image::ImageFormat::Png | image::ImageFormat::Jpeg)
    ) {
        return Err(SkipReason::ImageDecodeFailed);
    }
    let mut limits = image::Limits::default();
    limits.max_alloc = Some(MAX_ALLOC);
    reader.limits(limits);
    let mut decoder = reader.into_decoder().map_err(decode_error)?;
    use image::ImageDecoder;
    let (w, h) = decoder.dimensions();
    if u64::from(w) * u64::from(h) > configured.pixels() {
        return Err(SkipReason::ImageTooLarge);
    }
    let orientation = decoder.orientation().map_err(decode_error)?;
    let mut image = image::DynamicImage::from_decoder(decoder).map_err(decode_error)?;
    image.apply_orientation(orientation);
    Ok(image.into_rgba8())
}
fn header_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        if bytes.get(8..16)? != b"\0\0\0\rIHDR" {
            return None;
        }
        return Some((
            u32::from_be_bytes(bytes.get(16..20)?.try_into().ok()?),
            u32::from_be_bytes(bytes.get(20..24)?.try_into().ok()?),
        ));
    }
    if !bytes.starts_with(b"\xff\xd8") {
        return None;
    }
    let mut position = 2;
    while *bytes.get(position)? == 0xff {
        while *bytes.get(position)? == 0xff {
            position += 1;
        }
        let marker = *bytes.get(position)?;
        position += 1;
        if matches!(marker, 0xd9 | 0xda | 0x00) {
            return None;
        }
        if matches!(marker, 0xd8 | 0x01 | 0xd0..=0xd7) {
            continue;
        }
        let length = usize::from(u16::from_be_bytes(
            bytes.get(position..position + 2)?.try_into().ok()?,
        ));
        if length < 2 {
            return None;
        }
        if matches!(marker, 0xc0..=0xc3 | 0xc5..=0xc7 | 0xc9..=0xcb | 0xcd..=0xcf) {
            if length < 8 {
                return None;
            }
            return Some((
                u32::from(u16::from_be_bytes(
                    bytes.get(position + 5..position + 7)?.try_into().ok()?,
                )),
                u32::from(u16::from_be_bytes(
                    bytes.get(position + 3..position + 5)?.try_into().ok()?,
                )),
            ));
        }
        position += length;
    }
    None
}
fn decode_error(error: image::ImageError) -> SkipReason {
    if matches!(error, image::ImageError::Limits(_)) {
        SkipReason::ImageTooLarge
    } else {
        SkipReason::ImageDecodeFailed
    }
}
struct Request {
    image: RgbImage,
    reply: mpsc::SyncSender<Result<Vec<OcrLine>, OcrError>>,
}
struct Worker {
    requests: mpsc::SyncSender<Request>,
}
enum State {
    Cold,
    Ready(Worker),
    Unavailable,
    TimedOut,
}
pub struct ImagePipeline {
    models: Option<PathBuf>,
    upscale: bool,
    worker: Mutex<State>,
}
impl ImagePipeline {
    pub fn new(models: Option<PathBuf>, upscale: bool) -> Self {
        Self {
            models: resolve_models(models),
            upscale,
            worker: Mutex::new(State::Cold),
        }
    }
    fn recognize(&self, image: RgbImage, timeout: Duration) -> Result<Vec<OcrLine>, SkipReason> {
        let mut state = self.worker.lock().map_err(|_| SkipReason::OcrUnavailable)?;
        if matches!(*state, State::Cold) {
            let models = self.models.clone();
            let upscale = self.upscale;
            let (tx, rx) = mpsc::sync_channel::<Request>(1);
            std::thread::Builder::new()
                .name("hushpush-ocr".into())
                .spawn(move || {
                    let engine = models
                        .as_deref()
                        .ok_or(OcrError)
                        .and_then(PpOcrEngine::load);
                    while let Ok(request) = rx.recv() {
                        let result = match &engine {
                            Ok(engine) => {
                                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                                    recognize_image(engine, request.image, upscale)
                                }))
                                .unwrap_or(Err(OcrError))
                            }
                            Err(_) => Err(OcrError),
                        };
                        if request.reply.send(result).is_err() {
                            break;
                        }
                    }
                })
                .map_err(|_| SkipReason::OcrUnavailable)?;
            *state = State::Ready(Worker { requests: tx });
        }
        let worker = match &*state {
            State::Ready(worker) => worker,
            State::TimedOut => return Err(SkipReason::OcrTimeout),
            _ => return Err(SkipReason::OcrUnavailable),
        };
        let (tx, rx) = mpsc::sync_channel(1);
        if worker.requests.send(Request { image, reply: tx }).is_err() {
            *state = State::Unavailable;
            return Err(SkipReason::OcrUnavailable);
        }
        match rx.recv_timeout(timeout) {
            Ok(Ok(lines)) => Ok(lines),
            Ok(Err(_)) => Err(SkipReason::OcrUnavailable),
            Err(mpsc::RecvTimeoutError::Timeout) => {
                // Rust cannot safely kill an inference thread. Retire this worker and never accumulate replacements.
                *state = State::TimedOut;
                Err(SkipReason::OcrTimeout)
            }
            _ => {
                *state = State::Unavailable;
                Err(SkipReason::OcrUnavailable)
            }
        }
    }
    pub fn scan(
        &self,
        bytes: &[u8],
        path: String,
        origin: Origin<'_>,
        include_reserved: bool,
    ) -> Result<Vec<Finding>, SkipReason> {
        let mut config = Config::default();
        config.detectors.email.include_reserved_domains = include_reserved;
        config.detectors.phone.include_reserved = include_reserved;
        self.scan_with_config(bytes, path, origin, &config)
    }
    pub fn scan_with_config(
        &self,
        bytes: &[u8],
        path: String,
        origin: Origin<'_>,
        config: &Config,
    ) -> Result<Vec<Finding>, SkipReason> {
        let image =
            image::DynamicImage::ImageRgba8(decode_rgba_with_limits(bytes, &config.limits)?)
                .into_rgb8();
        let dimensions = image.dimensions();
        let lines = self.recognize(image, Duration::from_secs(config.limits.ocr_timeout_secs))?;
        findings_with_config(&lines, dimensions, path, origin, config)
    }
}
fn recognize_image(
    engine: &dyn OcrEngine,
    image: RgbImage,
    upscale: bool,
) -> Result<Vec<OcrLine>, OcrError> {
    let factor = if upscale && image.width().max(image.height()) < 2560 {
        2
    } else {
        1
    };
    let input = if factor == 2 {
        image::imageops::resize(
            &image,
            image.width() * 2,
            image.height() * 2,
            image::imageops::FilterType::Lanczos3,
        )
    } else {
        image
    };
    let mut lines = engine.recognize(&input)?;
    for line in &mut lines {
        for ch in &mut line.chars {
            ch.ch = normalize_char(ch.ch);
            for v in &mut ch.bbox {
                *v /= factor as f32;
            }
        }
        line.text = line.chars.iter().map(|c| c.ch).collect();
    }
    Ok(lines)
}
pub struct MappedText {
    pub text: Sensitive<String>,
    boxes: Vec<(Range<usize>, [f32; 4])>,
}
impl MappedText {
    pub fn from_lines(lines: &[OcrLine]) -> Self {
        let mut text = String::new();
        let mut boxes = Vec::new();
        let bounds = |line: &OcrLine| {
            line.chars
                .iter()
                .fold([f32::INFINITY, f32::INFINITY, 0f32, 0f32], |b, c| {
                    [
                        b[0].min(c.bbox[0]),
                        b[1].min(c.bbox[1]),
                        b[2].max(c.bbox[2]),
                        b[3].max(c.bbox[3]),
                    ]
                })
        };
        let coherent = |line: &OcrLine| {
            let b = bounds(line);
            let (low, high) = line
                .chars
                .iter()
                .fold((f32::INFINITY, 0f32), |(low, high), c| {
                    let center = (c.bbox[1] + c.bbox[3]) * 0.5;
                    (low.min(center), high.max(center))
                });
            high - low <= (b[3] - b[1]) * 0.5
        };
        let mut rows: Vec<([f32; 4], Vec<&OcrLine>)> = Vec::new();
        for line in lines.iter().filter(|line| !line.chars.is_empty()) {
            let b = bounds(line);
            if let Some((row, fragments)) = rows.iter_mut().find(|(r, fragments)| {
                let overlap = r[3].min(b[3]) - r[1].max(b[1]);
                coherent(line)
                    && fragments.iter().all(|fragment| coherent(fragment))
                    && overlap >= 0.5 * (r[3] - r[1]).min(b[3] - b[1])
                    && (r[3] - r[1]).max(b[3] - b[1]) <= 2.0 * (r[3] - r[1]).min(b[3] - b[1])
                    && (b[0] >= r[2] - 0.35 * (r[3] - r[1]).min(b[3] - b[1])
                        || r[0] >= b[2] - 0.35 * (r[3] - r[1]).min(b[3] - b[1]))
            }) {
                row[0] = row[0].min(b[0]);
                row[2] = row[2].max(b[2]);
                fragments.push(line);
            } else {
                rows.push((b, vec![line]));
            }
        }
        rows.sort_by(|a, b| a.0[1].total_cmp(&b.0[1]));
        let mut previous: Option<[f32; 4]> = None;
        for (_, mut fragments) in rows {
            fragments.sort_by(|a, b| bounds(a)[0].total_cmp(&bounds(b)[0]));
            for (index, line) in fragments.into_iter().enumerate() {
                let b = bounds(line);
                if let Some(last) = previous {
                    // OCR regions on one visual row are fragments, not physical lines.
                    let height = (b[3] - b[1]).max(last[3] - last[1]);
                    if index > 0 && b[0] - last[2] <= 3.0 * height {
                        let numeric_hyphens = |value: &str| {
                            value.contains('-')
                                && value.bytes().all(|c| c.is_ascii_digit() || c == b'-')
                        };
                        let previous_token = text.rsplit([' ', '\n']).next().unwrap_or("");
                        if !text.ends_with('-')
                            && !line.text.starts_with('-')
                            && !(numeric_hyphens(previous_token) && numeric_hyphens(&line.text))
                        {
                            text.push(' ');
                        }
                    } else {
                        text.push('\n');
                    }
                }
                previous = Some(b);
                for ch in &line.chars {
                    let start = text.len();
                    text.push(normalize_char(ch.ch));
                    boxes.push((start..text.len(), ch.bbox));
                }
            }
        }
        Self {
            text: Sensitive::new(text),
            boxes,
        }
    }
    pub fn location(&self, span: Range<usize>, size: (u32, u32)) -> Option<Location> {
        let boxes: Vec<_> = self
            .boxes
            .iter()
            .filter(|(r, _)| r.start < span.end && r.end > span.start)
            .map(|(_, b)| *b)
            .collect();
        padded_union(&boxes, size)
    }
}
pub fn padded_union(boxes: &[[f32; 4]], (width, height): (u32, u32)) -> Option<Location> {
    if boxes.is_empty() || boxes.iter().flatten().any(|v| !v.is_finite()) {
        return None;
    }
    let x = (boxes.iter().map(|b| b[0]).fold(f32::INFINITY, f32::min) - 8.)
        .floor()
        .clamp(0., width as f32) as u32;
    let y = (boxes.iter().map(|b| b[1]).fold(f32::INFINITY, f32::min) - 8.)
        .floor()
        .clamp(0., height as f32) as u32;
    let right = (boxes.iter().map(|b| b[2]).fold(0., f32::max) + 8.)
        .ceil()
        .clamp(x as f32, width as f32) as u32;
    let bottom = (boxes.iter().map(|b| b[3]).fold(0., f32::max) + 8.)
        .ceil()
        .clamp(y as f32, height as f32) as u32;
    Some(Location::Bbox {
        x,
        y,
        w: right - x,
        h: bottom - y,
    })
}
pub fn findings(
    lines: &[OcrLine],
    size: (u32, u32),
    path: String,
    origin: Origin<'_>,
    include_reserved: bool,
) -> Result<Vec<Finding>, SkipReason> {
    let mut config = Config::default();
    config.detectors.email.include_reserved_domains = include_reserved;
    config.detectors.phone.include_reserved = include_reserved;
    findings_with_config(lines, size, path, origin, &config)
}
fn findings_with_config(
    lines: &[OcrLine],
    size: (u32, u32),
    path: String,
    origin: Origin<'_>,
    config: &Config,
) -> Result<Vec<Finding>, SkipReason> {
    let mapped = MappedText::from_lines(lines);
    let detectors = config.detector_list();
    let mut findings = Vec::new();
    for detector in detectors {
        let Some(severity) = config.severity_for(detector.as_ref()) else {
            continue;
        };
        for found in detector.find(mapped.text.expose()) {
            let location = mapped
                .location(found.span.clone(), size)
                .ok_or(SkipReason::OcrUnavailable)?;
            let value = Sensitive::new(mapped.text.expose()[found.span.clone()].to_owned());
            findings.push((
                found.span.start,
                Finding {
                    fingerprint: None,
                    suppressed_by: None,
                    detector: detector.id().to_owned(),
                    detector_version: detector.version(),
                    category: detector.category(),
                    severity,
                    confidence: match found.confidence {
                        Confidence::High => Confidence::Medium,
                        other => other,
                    },
                    path: path.clone(),
                    blob_oid: origin.blob_oid.map(str::to_owned),
                    location,
                    preview: preview_value(detector.category(), detector.id(), value.expose()),
                    value,
                },
            ));
        }
    }
    findings.sort_by_key(|(start, _)| *start);
    Ok(findings.into_iter().map(|(_, f)| f).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn png(image: &RgbImage) -> Vec<u8> {
        let mut bytes = Cursor::new(Vec::new());
        image.write_to(&mut bytes, image::ImageFormat::Png).unwrap();
        bytes.into_inner()
    }

    #[test]
    fn supported_pixel_encodings_are_scanned_without_ocr_failures() {
        for bytes in [
            include_bytes!("fixtures/gray16.png").as_slice(),
            include_bytes!("fixtures/palette.png").as_slice(),
            include_bytes!("fixtures/gray-alpha.png").as_slice(),
            include_bytes!("fixtures/cmyk.jpg").as_slice(),
            include_bytes!("fixtures/progressive.jpg").as_slice(),
        ] {
            let pipeline = mock_pipeline(EmptyOcr);
            assert!(
                pipeline
                    .scan(bytes, "synthetic-image".into(), Origin::default(), true)
                    .unwrap()
                    .is_empty()
            );
            assert_eq!(decode(bytes).unwrap().dimensions(), (2, 3));
        }
        assert_eq!(
            decode(&png(&RgbImage::new(1, 1))).unwrap().dimensions(),
            (1, 1)
        );
    }
    struct EmptyOcr;
    impl OcrEngine for EmptyOcr {
        fn recognize(&self, _: &RgbImage) -> Result<Vec<OcrLine>, OcrError> {
            Ok(vec![])
        }
    }
    #[test]
    fn corrupt_images_fail_without_allocating_unbounded_rasters() {
        let original = png(&RgbImage::new(2, 3));
        let mut bad_crc = original.clone();
        bad_crc[29] ^= 1;
        for bytes in [b"".as_slice(), &original[..original.len() / 2], &bad_crc] {
            assert_eq!(decode(bytes).err(), Some(SkipReason::ImageDecodeFailed));
        }
        let mut bomb = original[..24].to_vec();
        bomb[16..20].copy_from_slice(&u32::MAX.to_be_bytes());
        bomb[20..24].copy_from_slice(&u32::MAX.to_be_bytes());
        assert_eq!(decode(&bomb).err(), Some(SkipReason::ImageTooLarge));
        // No scan data or complete frame is required to enforce the JPEG pixel limit.
        let huge_sof = b"\xff\xd8\xff\xe1\x00\x04xx\xff\xc2\x00\x08\x08\xff\xff\xff\xff\x03";
        assert_eq!(decode(huge_sof).err(), Some(SkipReason::ImageTooLarge));
        for end in 0..huge_sof.len() {
            let _ = decode(&huge_sof[..end]);
        }
    }
    #[test]
    fn orientation_six_is_applied_before_ocr() {
        struct Displayed;
        impl OcrEngine for Displayed {
            fn recognize(&self, img: &RgbImage) -> Result<Vec<OcrLine>, OcrError> {
                assert_eq!(img.dimensions(), (400, 100));
                assert!(
                    img.enumerate_pixels()
                        .any(|(x, y, pixel)| x > 10 && y > 20 && y < 74 && pixel[0] < 40)
                );
                Ok(vec![])
            }
        }
        let bytes = include_bytes!("fixtures/orientation6.jpg");
        assert_eq!(header_dimensions(bytes), Some((100, 400)));
        mock_pipeline(Displayed)
            .scan(bytes, "orientation6.jpg".into(), Origin::default(), true)
            .unwrap();
    }
    #[test]
    #[ignore = "requires HUSHPUSH_MODELS with verified OCR assets"]
    fn orientation_six_redacts_the_displayed_value() {
        let bytes = include_bytes!("fixtures/orientation6.jpg");
        let models = std::env::var_os("HUSHPUSH_MODELS").expect("HUSHPUSH_MODELS required");
        let pipeline = ImagePipeline::new(Some(PathBuf::from(models)), true);
        let found = pipeline
            .scan(bytes, "oriented.jpg".into(), Origin::default(), true)
            .unwrap();
        let email = found
            .iter()
            .find(|f| f.category == crate::detect::Category::Email)
            .expect("synthetic displayed email detected");
        let Location::Bbox { x, y, w, h } = email.location else {
            panic!("image geometry required")
        };
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../target")
            .canonicalize()
            .unwrap()
            .join(format!("orientation-redaction-{}", std::process::id()));
        let source = root.join("source");
        let output = root.join("redacted");
        std::fs::create_dir_all(&source).unwrap();
        std::fs::write(source.join("oriented.jpg"), bytes).unwrap();
        let mut config = Config::default();
        config.detectors.email.include_reserved_domains = true;
        let report = crate::redact::redact_with_config(&source, &output, &config).unwrap();
        assert_eq!(report.written, 1);
        assert!(report.skipped.is_empty());
        let redacted = std::fs::read(output.join("oriented.jpg.png")).unwrap();
        let image = decode(&redacted).unwrap();
        assert_eq!(image.dimensions(), (400, 100));
        for py in y..y + h {
            for px in x..x + w {
                assert_eq!(image.get_pixel(px, py).0, [0, 0, 0]);
            }
        }
        assert!(
            pipeline
                .scan(&redacted, "redacted.png".into(), Origin::default(), true)
                .unwrap()
                .is_empty()
        );
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn adjacent_regions_on_one_row_reach_detectors_with_geometry() {
        let make = |text: &str, x: f32, y: f32| OcrLine {
            text: text.into(),
            chars: text
                .chars()
                .enumerate()
                .map(|(i, ch)| OcrChar {
                    ch,
                    bbox: [x + i as f32 * 8., y, x + (i + 1) as f32 * 8., y + 12.],
                })
                .collect(),
        };
        let lines = [
            make("4242 4242", 10., 10.),
            make("4242 4242", 81., 10.),
            make("+49 30", 10., 40.),
            make("9876 5432", 57., 40.),
        ];
        let found = findings(
            &lines,
            (300, 100),
            "synthetic.png".into(),
            Origin::default(),
            true,
        )
        .unwrap();
        assert!(found.len() == 2);
        assert!(
            found
                .iter()
                .any(|f| f.category == crate::detect::Category::PaymentCard)
        );
        assert!(
            found
                .iter()
                .any(|f| f.category == crate::detect::Category::Phone)
        );
        let separate = [make("4242 4242", 10., 10.), make("4242 4242", 10., 40.)];
        assert!(
            findings(
                &separate,
                (300, 100),
                "synthetic.png".into(),
                Origin::default(),
                true
            )
            .unwrap()
            .is_empty()
        );
    }
    #[test]
    fn nfkc_preserves_single_char_geometry() {
        assert_eq!(normalize_char('\u{ff08}'), '(');
        assert_eq!(normalize_char('\u{ff21}'), 'A');
        assert_eq!(normalize_char('\u{fb01}'), '\u{fb01}');
        assert_eq!(normalize_char('\u{e9}'), '\u{e9}');
        assert_eq!(normalize_char('\u{337f}'), '\u{337f}');
    }
    #[test]
    fn unicode_byte_offsets_newlines_and_padding_are_mapped() {
        let lines = vec![
            OcrLine {
                text: "\u{e9}\u{ff21}".into(),
                chars: vec![
                    OcrChar {
                        ch: '\u{e9}',
                        bbox: [2., 3., 4., 5.],
                    },
                    OcrChar {
                        ch: '\u{ff21}',
                        bbox: [20., 10., 29., 19.],
                    },
                ],
            },
            OcrLine {
                text: "x".into(),
                chars: vec![OcrChar {
                    ch: 'x',
                    bbox: [40., 10., 50., 20.],
                }],
            },
        ];
        let mapped = MappedText::from_lines(&lines);
        assert_eq!(mapped.text.expose(), "\u{e9}A\nx");
        assert_eq!(
            mapped.location(2..3, (45, 25)),
            Some(Location::Bbox {
                x: 12,
                y: 2,
                w: 25,
                h: 23
            })
        );
        assert_eq!(mapped.location(3..4, (45, 25)), None);
        assert_eq!(
            mapped.location(0..5, (45, 25)),
            Some(Location::Bbox {
                x: 0,
                y: 0,
                w: 45,
                h: 25
            })
        );
    }
    #[test]
    fn bounded_decode_rejects_malformed_and_oversized_headers() {
        assert_eq!(
            decode(b"\x89PNG\r\n\x1a\n").err(),
            Some(SkipReason::ImageDecodeFailed)
        );
        assert_eq!(
            decode(&vec![0; MAX_IMAGE_BYTES as usize + 1]).err(),
            Some(SkipReason::ImageTooLarge)
        );
        // A valid PNG header/CRC with impossible dimensions fails before raster allocation.
        let mut bytes = png(&RgbImage::new(1, 1));
        bytes[16..20].copy_from_slice(&100_000_u32.to_be_bytes());
        bytes[20..24].copy_from_slice(&100_000_u32.to_be_bytes());
        // CRC is recalculated with a tiny local implementation to exercise the pixel gate.
        let mut crc = !0_u32;
        for b in &bytes[12..29] {
            crc ^= u32::from(*b);
            for _ in 0..8 {
                crc = (crc >> 1) ^ (0xedb88320_u32.wrapping_mul(crc & 1));
            }
        }
        bytes[29..33].copy_from_slice(&(!crc).to_be_bytes());
        assert_eq!(decode(&bytes).err(), Some(SkipReason::ImageTooLarge));
    }
    #[test]
    fn missing_models_is_incomplete_and_text_remains_available() {
        let pipeline =
            ImagePipeline::new(Some(PathBuf::from("synthetic-nonexistent-models")), true);
        let bytes = png(&RgbImage::new(8, 8));
        for _ in 0..2 {
            assert_eq!(
                pipeline
                    .scan(&bytes, "fake.png".into(), Origin::default(), true)
                    .err(),
                Some(SkipReason::OcrUnavailable)
            );
        }
        let mut report = crate::report::Report::default();
        crate::source::scan_bytes(
            &mut report,
            "fake.log".into(),
            b"demo@example.com",
            Origin::default(),
            true,
        );
        assert_eq!(report.coverage.scanned, 1);
        assert_eq!(report.findings.len(), 1);
        for path in ["a.png", "b.png"] {
            crate::source::skip(&mut report, path.into(), SkipReason::OcrUnavailable);
        }
        let mut output = Vec::new();
        report.write_terminal(&mut output).unwrap();
        let output = String::from_utf8(output).unwrap();
        assert_eq!(output.matches("OCR unavailable:").count(), 1);
        assert!(report.coverage.incomplete);
        assert!(!output.contains("demo@example.com"));
    }
    #[test]
    fn timed_out_worker_is_retired_without_replacement() {
        let (tx, rx) = mpsc::sync_channel::<Request>(1);
        let worker = std::thread::spawn(move || {
            let request = rx.recv().unwrap();
            std::thread::sleep(Duration::from_millis(30));
            let _ = request.reply.send(Ok(vec![]));
        });
        let pipeline = ImagePipeline {
            models: None,
            upscale: false,
            worker: Mutex::new(State::Ready(Worker { requests: tx })),
        };
        for _ in 0..2 {
            assert_eq!(
                pipeline
                    .recognize(RgbImage::new(1, 1), Duration::from_millis(1))
                    .err(),
                Some(SkipReason::OcrTimeout)
            );
        }
        worker.join().unwrap();
    }
    fn mock_pipeline(engine: impl OcrEngine + 'static) -> ImagePipeline {
        let (tx, rx) = mpsc::sync_channel::<Request>(1);
        std::thread::spawn(move || {
            while let Ok(request) = rx.recv() {
                let result = engine.recognize(&request.image);
                if request.reply.send(result).is_err() {
                    break;
                }
            }
        });
        ImagePipeline {
            models: None,
            upscale: false,
            worker: Mutex::new(State::Ready(Worker { requests: tx })),
        }
    }
    fn assert_mock_coverage(pipeline: &ImagePipeline, reason: SkipReason) {
        use crate::{ExitCode, config::PolicyAction, report::Report};
        let bytes = png(&RgbImage::new(8, 8));
        for action in [PolicyAction::Block, PolicyAction::Warn] {
            let mut config = Config::default();
            config.limits.ocr_timeout_secs = 1;
            config.policy.on_incomplete = action;
            let mut report = Report {
                config,
                ..Report::default()
            };
            let config = report.config.clone();
            crate::source::scan_bytes_with_image(
                &mut report,
                "synthetic.png".into(),
                &bytes,
                Origin::default(),
                false,
                &|bytes, path, origin, _| pipeline.scan_with_config(bytes, path, origin, &config),
            );
            assert_eq!(report.coverage.skipped.len(), 1);
            assert_eq!(report.coverage.skipped[0].reason, reason);
            assert_eq!(
                report.exit_code(),
                if action == PolicyAction::Block {
                    ExitCode::IncompleteCoverage
                } else {
                    ExitCode::NoBlockingFindings
                },
            );
            let mut output = Vec::new();
            report.write_terminal(&mut output).unwrap();
            let output = String::from_utf8(output).unwrap();
            assert!(output.contains("Coverage incomplete"));
            assert!(output.contains(reason.as_str()));
        }
    }
    #[test]
    fn mock_engine_error_obeys_incomplete_policy() {
        struct Failure;
        impl OcrEngine for Failure {
            fn recognize(&self, _: &RgbImage) -> Result<Vec<OcrLine>, OcrError> {
                Err(OcrError)
            }
        }
        assert_mock_coverage(&mock_pipeline(Failure), SkipReason::OcrUnavailable);
    }
    #[test]
    fn mock_engine_timeout_obeys_configured_one_second_limit_and_policy() {
        struct Slow;
        impl OcrEngine for Slow {
            fn recognize(&self, _: &RgbImage) -> Result<Vec<OcrLine>, OcrError> {
                std::thread::sleep(Duration::from_millis(1200));
                Ok(vec![])
            }
        }
        assert_mock_coverage(&mock_pipeline(Slow), SkipReason::OcrTimeout);
    }
    #[test]
    fn configured_decode_limits_bound_bytes_and_pixels() {
        let bytes = png(&RgbImage::new(1024, 1024));
        let mut limits = Limits {
            max_image_megapixels: 1,
            ..Limits::default()
        };
        assert_eq!(
            decode_rgba_with_limits(&bytes, &limits).err(),
            Some(SkipReason::ImageTooLarge)
        );
        limits.max_image_megapixels = 2;
        assert!(decode_rgba_with_limits(&bytes, &limits).is_ok());
        limits.max_image_mib = 0;
        assert_eq!(
            decode_rgba_with_limits(&bytes, &limits).err(),
            Some(SkipReason::ImageTooLarge)
        );
    }
    #[test]
    fn disabled_detector_does_not_attempt_geometry_mapping() {
        let lines = [OcrLine {
            text: "demo@example.com".into(),
            chars: "demo@example.com"
                .chars()
                .map(|ch| OcrChar {
                    ch,
                    bbox: [f32::NAN; 4],
                })
                .collect(),
        }];
        let mut config = Config::default();
        config.detectors.email.include_reserved_domains = true;
        config.detectors.email.severity = crate::config::DetectorSeverity::Off;
        let found = findings_with_config(
            &lines,
            (100, 100),
            "synthetic.png".into(),
            Origin::default(),
            &config,
        )
        .unwrap();
        assert!(found.is_empty());
    }
    #[test]
    fn mock_ocr_maps_upscale_boxes_back_and_caps_confidence() {
        struct Mock;
        impl OcrEngine for Mock {
            fn recognize(&self, img: &RgbImage) -> Result<Vec<OcrLine>, OcrError> {
                assert_eq!(img.dimensions(), (200, 100));
                let chars = "demo@example.com"
                    .chars()
                    .enumerate()
                    .map(|(i, ch)| OcrChar {
                        ch,
                        bbox: [(i * 8) as f32, 10., (i * 8 + 8) as f32, 30.],
                    })
                    .collect();
                Ok(vec![OcrLine {
                    text: "demo@example.com".into(),
                    chars,
                }])
            }
        }
        let lines = recognize_image(&Mock, RgbImage::new(100, 50), true).unwrap();
        let result = findings(
            &lines,
            (100, 50),
            "demo.png".into(),
            Origin::default(),
            true,
        )
        .unwrap();
        assert_eq!(
            result[0].location,
            Location::Bbox {
                x: 0,
                y: 0,
                w: 72,
                h: 23
            }
        );
        assert_eq!(result[0].confidence, Confidence::Medium);
    }
}
