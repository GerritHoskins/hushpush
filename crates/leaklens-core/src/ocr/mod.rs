//! Bounded image decoding and local OCR. Scans never acquire models or runtimes.
use crate::{
    Sensitive,
    detect::{CardDetector, Confidence, Detector, EmailDetector},
    report::{Finding, Location, SkipReason, preview},
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
pub const MAX_IMAGE_BYTES: u64 = 50_000_000;
pub const MAX_PIXELS: u64 = 50_000_000;
pub const MAX_ALLOC: u64 = 512 * 1024 * 1024;
const TIMEOUT: Duration = Duration::from_secs(20);
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
    /// [left, top, right, bottom] in original image pixels.
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
        .or_else(|| std::env::var_os("LEAKLENS_MODELS").map(PathBuf::from))
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
    if bytes.len() as u64 > MAX_IMAGE_BYTES {
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
    let decoder = reader.into_decoder().map_err(decode_error)?;
    use image::ImageDecoder;
    let (w, h) = decoder.dimensions();
    if u64::from(w) * u64::from(h) > MAX_PIXELS {
        return Err(SkipReason::ImageTooLarge);
    }
    image::DynamicImage::from_decoder(decoder)
        .map(|i| i.into_rgb8())
        .map_err(decode_error)
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
    timeout: Duration,
}
impl ImagePipeline {
    pub fn new(models: Option<PathBuf>, upscale: bool) -> Self {
        Self {
            models: resolve_models(models),
            upscale,
            worker: Mutex::new(State::Cold),
            timeout: TIMEOUT,
        }
    }
    fn recognize(&self, image: RgbImage) -> Result<Vec<OcrLine>, SkipReason> {
        let mut state = self.worker.lock().map_err(|_| SkipReason::OcrUnavailable)?;
        if matches!(*state, State::Cold) {
            let models = self.models.clone();
            let upscale = self.upscale;
            let (tx, rx) = mpsc::sync_channel::<Request>(1);
            std::thread::Builder::new()
                .name("leaklens-ocr".into())
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
        match rx.recv_timeout(self.timeout) {
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
        let image = decode(bytes)?;
        let dimensions = image.dimensions();
        let lines = self.recognize(image)?;
        findings(&lines, dimensions, path, origin, include_reserved)
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
        for (i, line) in lines.iter().enumerate() {
            if i > 0 {
                text.push('\n');
            }
            for ch in &line.chars {
                let start = text.len();
                text.push(normalize_char(ch.ch));
                boxes.push((start..text.len(), ch.bbox));
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
    let mapped = MappedText::from_lines(lines);
    let email = EmailDetector::new(include_reserved);
    let detectors: [&dyn Detector; 2] = [&email, &CardDetector];
    let mut findings = Vec::new();
    for detector in detectors {
        for found in detector.find(mapped.text.expose()) {
            let location = mapped
                .location(found.span.clone(), size)
                .ok_or(SkipReason::OcrUnavailable)?;
            let value = Sensitive::new(mapped.text.expose()[found.span.clone()].to_owned());
            findings.push((
                found.span.start,
                Finding {
                    detector: detector.id(),
                    detector_version: detector.version(),
                    category: detector.category(),
                    severity: detector.severity(),
                    confidence: match found.confidence {
                        Confidence::High => Confidence::Medium,
                        other => other,
                    },
                    path: path.clone(),
                    blob_oid: origin.blob_oid.map(str::to_owned),
                    location,
                    preview: preview(detector.category(), value.expose().chars().count()),
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
            timeout: Duration::from_millis(1),
        };
        for _ in 0..2 {
            assert_eq!(
                pipeline.recognize(RgbImage::new(1, 1)).err(),
                Some(SkipReason::OcrTimeout)
            );
        }
        worker.join().unwrap();
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
