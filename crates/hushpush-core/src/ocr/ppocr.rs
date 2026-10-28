//! PP-OCRv5 mobile CPU inference, copied and adapted from the synthetic S1b runner.
use super::{OcrChar, OcrEngine, OcrError, OcrLine, normalize_char};
use image::{GrayImage, Luma, Rgb, RgbImage};
use imageproc::{
    contours::{BorderType, find_contours},
    geometric_transformations::{Interpolation, Projection, warp_into},
    geometry::min_area_rect,
};
use ort::{
    session::{Session, builder::GraphOptimizationLevel},
    value::Tensor,
};
use std::{path::Path, sync::Mutex};
type Quad = [(f32, f32); 4];
struct Sessions {
    det: Session,
    rec: Session,
    dictionary: Vec<String>,
}
pub struct PpOcrEngine {
    sessions: Mutex<Sessions>,
}
impl PpOcrEngine {
    pub fn load(models: &Path) -> Result<Self, OcrError> {
        let det = std::fs::read(models.join("mobile_det_inference.onnx")).map_err(|_| OcrError)?;
        let rec = std::fs::read(models.join("mobile_rec_inference.onnx")).map_err(|_| OcrError)?;
        let dict = std::fs::read(models.join("mobile_dict.txt")).map_err(|_| OcrError)?;
        Self::from_bytes(&det, &rec, &dict)
    }
    #[cfg(feature = "bundled-models")]
    pub fn bundled() -> Result<Self, OcrError> {
        Self::from_bytes(
            include_bytes!(concat!(env!("OUT_DIR"), "/mobile_det_inference.onnx")),
            include_bytes!(concat!(env!("OUT_DIR"), "/mobile_rec_inference.onnx")),
            include_bytes!(concat!(env!("OUT_DIR"), "/mobile_dict.txt")),
        )
    }
    fn from_bytes(det: &[u8], rec: &[u8], dict: &[u8]) -> Result<Self, OcrError> {
        // Session initialization may panic. Never expose the panic payload.
        std::panic::catch_unwind(|| {
            let mut dictionary = vec![String::new()];
            dictionary.extend(
                std::str::from_utf8(dict)
                    .map_err(|_| OcrError)?
                    .lines()
                    .map(str::to_owned),
            );
            dictionary.push(" ".into());
            Ok(Self {
                sessions: Mutex::new(Sessions {
                    det: session(det)?,
                    rec: session(rec)?,
                    dictionary,
                }),
            })
        })
        .map_err(|_| OcrError)?
    }
}

impl OcrEngine for PpOcrEngine {
    fn recognize(&self, image: &RgbImage) -> Result<Vec<OcrLine>, OcrError> {
        self.sessions.lock().map_err(|_| OcrError)?.run(image)
    }
}
fn session(bytes: &[u8]) -> Result<Session, OcrError> {
    Ok(Session::builder()?
        .with_intra_threads(4)?
        .with_inter_threads(1)?
        .with_parallel_execution(false)?
        .with_optimization_level(GraphOptimizationLevel::Level3)?
        .commit_from_memory(bytes)?)
}
fn points_bbox(points: &[(f32, f32)]) -> [f32; 4] {
    [
        points.iter().map(|p| p.0).fold(f32::INFINITY, f32::min),
        points.iter().map(|p| p.1).fold(f32::INFINITY, f32::min),
        points.iter().map(|p| p.0).fold(f32::NEG_INFINITY, f32::max),
        points.iter().map(|p| p.1).fold(f32::NEG_INFINITY, f32::max),
    ]
}
fn crop_dimensions(width: u32, height: u32) -> Result<(), OcrError> {
    if width == 0 || height == 0 || u64::from(width) * u64::from(height) > 16_777_216 {
        return Err(OcrError);
    }
    Ok(())
}
fn recognition_width(width: u32, height: u32) -> Result<u32, OcrError> {
    if height == 0 {
        return Err(OcrError);
    }
    let width = (48_u64 * u64::from(width)).div_ceil(u64::from(height));
    // Limit aspect-ratio amplification independently of the decoded raster budget.
    if width == 0 || width > 16_384 {
        return Err(OcrError);
    }
    Ok(width as u32)
}
fn tensor_image(image: &RgbImage, det: bool) -> Vec<f32> {
    let plane = (image.width() * image.height()) as usize;
    let mut data = vec![0.; plane * 3];
    for (i, pixel) in image.pixels().enumerate() {
        for c in 0..3 {
            // Paddle/RapidOCR decode into BGR, and retain that order for both networks.
            let value = pixel[2 - c] as f32 / 255.;
            data[c * plane + i] = if det {
                (value - [0.485, 0.456, 0.406][c]) / [0.229, 0.224, 0.225][c]
            } else {
                (value - 0.5) / 0.5
            };
        }
    }
    data
}
fn distance(a: (f32, f32), b: (f32, f32)) -> f32 {
    (a.0 - b.0).hypot(a.1 - b.1)
}
fn ordered(points: Quad) -> Quad {
    let mut p = points;
    p.sort_by(|a, b| a.0.total_cmp(&b.0));
    let (mut left, mut right) = ([p[0], p[1]], [p[2], p[3]]);
    left.sort_by(|a, b| a.1.total_cmp(&b.1));
    right.sort_by(|a, b| a.1.total_cmp(&b.1));
    [left[0], right[0], right[1], left[1]]
}
fn unclip(q: Quad, ratio: f32) -> Quad {
    let w = distance(q[0], q[1]);
    let h = distance(q[1], q[2]);
    let d = w * h * ratio / (2. * (w + h));
    let u = ((q[1].0 - q[0].0) / w, (q[1].1 - q[0].1) / w);
    let v = ((q[3].0 - q[0].0) / h, (q[3].1 - q[0].1) / h);
    [(-1., -1.), (1., -1.), (1., 1.), (-1., 1.)].map_with_index(|i, (su, sv)| {
        (
            q[i].0 + d * (su * u.0 + sv * v.0),
            q[i].1 + d * (su * u.1 + sv * v.1),
        )
    })
}
trait MapIndex<T, const N: usize> {
    fn map_with_index<U>(self, f: impl FnMut(usize, T) -> U) -> [U; N];
}
impl<T, const N: usize> MapIndex<T, N> for [T; N] {
    fn map_with_index<U>(self, mut f: impl FnMut(usize, T) -> U) -> [U; N] {
        let mut i = 0;
        self.map(|v| {
            let r = f(i, v);
            i += 1;
            r
        })
    }
}
fn inside(p: (f32, f32), q: Quad) -> bool {
    let mut positive = false;
    let mut negative = false;
    for i in 0..4 {
        let a = q[i];
        let b = q[(i + 1) % 4];
        let c = (b.0 - a.0) * (p.1 - a.1) - (b.1 - a.1) * (p.0 - a.0);
        positive |= c > 0.;
        negative |= c < 0.;
    }
    !(positive && negative)
}
fn boxes(prob: &[f32], w: u32, h: u32, original: (u32, u32)) -> Result<Vec<Quad>, OcrError> {
    let mask = GrayImage::from_fn(w, h, |x, y| {
        Luma([if prob[(y * w + x) as usize] > 0.3 {
            255
        } else {
            0
        }])
    });
    let mut result = Vec::new();
    let mut candidates = 0;
    for contour in find_contours::<i32>(&mask)
        .into_iter()
        .filter(|c| c.border_type == BorderType::Outer)
    {
        candidates += 1;
        if candidates > 1000 {
            return Err(OcrError);
        }
        if contour.points.len() < 3 {
            continue;
        }
        let q = ordered(min_area_rect(&contour.points).map(|p| (p.x as f32, p.y as f32)));
        if distance(q[0], q[1]).min(distance(q[1], q[2])) < 3. {
            continue;
        }
        let minx = q
            .iter()
            .map(|p| p.0)
            .fold(f32::INFINITY, f32::min)
            .floor()
            .max(0.) as u32;
        let maxx = q
            .iter()
            .map(|p| p.0)
            .fold(0., f32::max)
            .ceil()
            .min((w - 1) as f32) as u32;
        let miny = q
            .iter()
            .map(|p| p.1)
            .fold(f32::INFINITY, f32::min)
            .floor()
            .max(0.) as u32;
        let maxy = q
            .iter()
            .map(|p| p.1)
            .fold(0., f32::max)
            .ceil()
            .min((h - 1) as f32) as u32;
        let mut sum = 0.;
        let mut n = 0;
        for y in miny..=maxy {
            for x in minx..=maxx {
                if inside((x as f32, y as f32), q) {
                    sum += prob[(y * w + x) as usize];
                    n += 1;
                }
            }
        }
        if n == 0 || sum / (n as f32) < 0.6 {
            continue;
        }
        let q = unclip(q, 1.5);
        let q = q.map(|(x, y)| {
            (
                (x * original.0 as f32 / w as f32)
                    .round()
                    .clamp(0., (original.0 - 1) as f32),
                (y * original.1 as f32 / h as f32)
                    .round()
                    .clamp(0., (original.1 - 1) as f32),
            )
        });
        if distance(q[0], q[1]).min(distance(q[1], q[2])) >= 4. {
            result.push(q);
        }
    }
    result.sort_by(|a, b| a[0].1.total_cmp(&b[0].1).then(a[0].0.total_cmp(&b[0].0)));
    // Adjacent detections can have slightly different top coordinates within the same row.
    for i in 1..result.len() {
        let mut j = i;
        while j > 0
            && (result[j][0].1 - result[j - 1][0].1).abs() < 10.
            && result[j][0].0 < result[j - 1][0].0
        {
            result.swap(j, j - 1);
            j -= 1;
        }
    }
    Ok(result)
}
fn ctc(data: &[f32], steps: usize, classes: usize) -> Vec<(usize, usize, usize)> {
    let mut result: Vec<(usize, usize, usize)> = Vec::new();
    let mut previous = 0;
    for t in 0..steps {
        let row = &data[t * classes..(t + 1) * classes];
        let token = row
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.total_cmp(b.1))
            .map_or(0, |p| p.0);
        if token != 0 {
            if token != previous {
                result.push((token, t, t));
            } else if let Some(last) = result.last_mut() {
                last.2 = t;
            }
        }
        previous = token;
    }
    result
}
impl Sessions {
    fn run(&mut self, image: &RgbImage) -> Result<Vec<OcrLine>, OcrError> {
        let scale = (1920_f32 / image.width().max(image.height()) as f32).min(1.);
        let w = ((image.width() as f32 * scale / 32.).round() as u32 * 32).max(32);
        let h = ((image.height() as f32 * scale / 32.).round() as u32 * 32).max(32);
        let resized = linear_resize(image, w, h);
        let input =
            Tensor::from_array(([1, 3, h as usize, w as usize], tensor_image(&resized, true)))?;
        let output = self.det.run(ort::inputs![input])?;
        let (shape, prob) = output[0].try_extract_tensor::<f32>()?;
        if shape.len() != 4 {
            return Err(OcrError);
        }
        let quads = boxes(prob, shape[3] as u32, shape[2] as u32, image.dimensions())?;
        drop(output);
        let mut lines = Vec::new();
        for quad in quads {
            let width = distance(quad[0], quad[1]).max(distance(quad[3], quad[2])) as u32;
            let height = distance(quad[0], quad[3]).max(distance(quad[1], quad[2])) as u32;
            if width == 0 || height == 0 {
                continue;
            }
            crop_dimensions(width, height)?;
            let target = [
                (0., 0.),
                ((width - 1) as f32, 0.),
                ((width - 1) as f32, (height - 1) as f32),
                (0., (height - 1) as f32),
            ];
            let transform = Projection::from_control_points(quad, target).ok_or(OcrError)?;
            let mut crop = RgbImage::new(width, height);
            warp_into(
                image,
                transform,
                Interpolation::Bilinear,
                imageproc::geometric_transformations::Border::Constant(Rgb([0, 0, 0])),
                &mut crop,
            );
            let mut inverse = transform.invert();
            if height as f32 / width as f32 >= 1.5 {
                crop = image::imageops::rotate270(&crop);
                let rotation = Projection::from_control_points(
                    [
                        (0., 0.),
                        ((height - 1) as f32, 0.),
                        ((height - 1) as f32, (width - 1) as f32),
                        (0., (width - 1) as f32),
                    ],
                    [
                        (width as f32 - 1., 0.),
                        (width as f32 - 1., height as f32 - 1.),
                        (0., height as f32 - 1.),
                        (0., 0.),
                    ],
                )
                .ok_or(OcrError)?;
                inverse = rotation.and_then(inverse);
            }
            let rec_width = recognition_width(crop.width(), crop.height())?;
            let padded_width = rec_width.max(320);
            let rec_image = linear_resize(&crop, rec_width, 48);
            let values = tensor_image(&rec_image, false);
            let mut padded = vec![0.; (3 * 48 * padded_width) as usize];
            for c in 0..3 {
                for y in 0..48 {
                    let src = (c * 48 * rec_width + y * rec_width) as usize;
                    let dst = (c * 48 * padded_width + y * padded_width) as usize;
                    padded[dst..dst + rec_width as usize]
                        .copy_from_slice(&values[src..src + rec_width as usize]);
                }
            }
            let input = Tensor::from_array(([1, 3, 48, padded_width as usize], padded))?;
            let output = self.rec.run(ort::inputs![input])?;
            let (shape, data) = output[0].try_extract_tensor::<f32>()?;
            if shape.len() != 3 {
                return Err(OcrError);
            }
            let steps = shape[1] as usize;
            let classes = shape[2] as usize;
            if classes != self.dictionary.len() {
                return Err(OcrError);
            }
            let tokens = ctc(data, steps, classes);
            let mut text = String::new();
            let mut chars = Vec::new();
            for (i, &(token, start, end)) in tokens.iter().enumerate() {
                let center = (start + end + 1) as f32 / 2.;
                let left = if i == 0 {
                    0.
                } else {
                    (center + (tokens[i - 1].1 + tokens[i - 1].2 + 1) as f32 / 2.) / 2.
                };
                let right = if i + 1 == tokens.len() {
                    rec_width as f32 / padded_width as f32 * steps as f32
                } else {
                    (center + (tokens[i + 1].1 + tokens[i + 1].2 + 1) as f32 / 2.) / 2.
                };
                let to_x = |t: f32| {
                    (t / steps as f32 * padded_width as f32 / rec_width as f32
                        * crop.width() as f32)
                        .clamp(0., crop.width() as f32)
                };
                let points = [
                    (to_x(left), 0.),
                    (to_x(right), 0.),
                    (to_x(right), crop.height() as f32),
                    (to_x(left), crop.height() as f32),
                ]
                .map(|p| inverse * p);
                let value = &self.dictionary[token];
                let bbox = points_bbox(&points);
                for ch in value.chars() {
                    let ch = normalize_char(ch);
                    text.push(ch);
                    chars.push(OcrChar { ch, bbox });
                }
            }
            if text.is_empty() {
                continue;
            }
            lines.push(OcrLine { text, chars });
        }
        Ok(lines)
    }
}
// OpenCV INTER_LINEAR uses half-pixel sampling without downscale antialiasing.
fn linear_resize(image: &RgbImage, width: u32, height: u32) -> RgbImage {
    RgbImage::from_fn(width, height, |x, y| {
        let sx = ((x as f32 + 0.5) * image.width() as f32 / width as f32 - 0.5).max(0.);
        let sy = ((y as f32 + 0.5) * image.height() as f32 / height as f32 - 0.5).max(0.);
        let x0 = (sx as u32).min(image.width() - 1);
        let x1 = (x0 + 1).min(image.width() - 1);
        let y0 = (sy as u32).min(image.height() - 1);
        let y1 = (y0 + 1).min(image.height() - 1);
        let ax = sx - x0 as f32;
        let ay = sy - y0 as f32;
        let mut out = [0; 3];
        for (c, channel) in out.iter_mut().enumerate() {
            let top = image.get_pixel(x0, y0)[c] as f32 * (1. - ax)
                + image.get_pixel(x1, y0)[c] as f32 * ax;
            let bottom = image.get_pixel(x0, y1)[c] as f32 * (1. - ax)
                + image.get_pixel(x1, y1)[c] as f32 * ax;
            *channel = (top * (1. - ay) + bottom * ay).round().clamp(0., 255.) as u8;
        }
        Rgb(out)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn excess_detector_candidates_fail_instead_of_silently_truncating() {
        let (w, h) = (256, 256);
        let prob: Vec<_> = (0..h)
            .flat_map(|y| (0..w).map(move |x| if x % 6 < 3 && y % 6 < 3 { 1.0 } else { 0.0 }))
            .collect();
        assert!(boxes(&prob, w, h, (w, h)).is_err());
    }
    #[test]
    fn recognition_allocations_are_bounded_even_for_extreme_aspect_ratios() {
        assert!(crop_dimensions(u32::MAX, u32::MAX).is_err());
        assert!(crop_dimensions(50_000_000, 1).is_err());
        assert!(recognition_width(10_000_000, 4).is_err());
        assert!(recognition_width(1, 0).is_err());
        assert_eq!(recognition_width(800, 48).unwrap(), 800);
    }
    #[test]
    fn ctc_collapses_runs_but_preserves_blank_separated_repeats() {
        let data = [0., 1., 0., 0., 1., 0., 1., 0., 0., 0., 1., 0., 0., 0., 1.];
        assert_eq!(ctc(&data, 5, 3), vec![(1, 0, 1), (1, 3, 3), (2, 4, 4)]);
    }
    #[test]
    fn unclip_increases_rectangle_by_area_perimeter_distance() {
        let q = [(0., 0.), (20., 0.), (20., 10.), (0., 10.)];
        let r = unclip(q, 1.5);
        assert_eq!(r, [(-5., -5.), (25., -5.), (25., 15.), (-5., 15.)]);
    }
    #[test]
    fn sensitive_never_formats_contents() {
        let s = crate::Sensitive::new("synthetic-value");
        assert_eq!(format!("{s:?} {s}"), "[redacted] [redacted]");
    }
}
