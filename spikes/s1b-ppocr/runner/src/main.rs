use anyhow::{Context, Result, ensure};
use image::{GrayImage, Luma, Rgb, RgbImage};
use imageproc::{
    contours::{BorderType, find_contours},
    geometric_transformations::{Interpolation, Projection, warp_into},
    geometry::min_area_rect,
};
use ort::{
    execution_providers::CPUExecutionProvider,
    session::{Session, builder::GraphOptimizationLevel},
    value::Tensor,
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
    time::Instant,
};

struct Sensitive<T>(T);
impl<T> std::fmt::Debug for Sensitive<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("[redacted]")
    }
}
impl<T> std::fmt::Display for Sensitive<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("[redacted]")
    }
}

type Quad = [(f32, f32); 4];
#[derive(Deserialize)]
struct Manifest {
    images: Vec<Entry>,
}
#[derive(Deserialize)]
struct Entry {
    id: String,
}
struct Engine {
    det: Session,
    rec: Session,
    dictionary: Vec<String>,
    max_side: u32,
}
fn session(path: &Path, threads: usize) -> Result<Session> {
    Ok(Session::builder()?
        .with_execution_providers([CPUExecutionProvider::default().build()])?
        .with_intra_threads(threads)?
        .with_inter_threads(1)?
        .with_parallel_execution(false)?
        .with_optimization_level(GraphOptimizationLevel::Level3)?
        .commit_from_file(path)?)
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
fn boxes(prob: &[f32], w: u32, h: u32, original: (u32, u32)) -> Vec<Quad> {
    let mask = GrayImage::from_fn(w, h, |x, y| {
        Luma([if prob[(y * w + x) as usize] > 0.3 {
            255
        } else {
            0
        }])
    });
    let mut result = Vec::new();
    for contour in find_contours::<i32>(&mask)
        .into_iter()
        .filter(|c| c.border_type == BorderType::Outer)
        .take(1000)
    {
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
    result
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
impl Engine {
    fn run(&mut self, image: &RgbImage) -> Result<Sensitive<Value>> {
        let scale = (self.max_side as f32 / image.width().max(image.height()) as f32).min(1.);
        let w = ((image.width() as f32 * scale / 32.).round() as u32 * 32).max(32);
        let h = ((image.height() as f32 * scale / 32.).round() as u32 * 32).max(32);
        let resized = linear_resize(image, w, h);
        let input =
            Tensor::from_array(([1, 3, h as usize, w as usize], tensor_image(&resized, true)))?;
        let output = self.det.run(ort::inputs![input])?;
        let (shape, prob) = output[0].try_extract_tensor::<f32>()?;
        ensure!(shape.len() == 4, "detector output shape mismatch");
        let quads = boxes(prob, shape[3] as u32, shape[2] as u32, image.dimensions());
        drop(output);
        let mut lines = Vec::new();
        for quad in quads {
            let width = distance(quad[0], quad[1]).max(distance(quad[3], quad[2])) as u32;
            let height = distance(quad[0], quad[3]).max(distance(quad[1], quad[2])) as u32;
            if width == 0 || height == 0 {
                continue;
            }
            let target = [
                (0., 0.),
                ((width - 1) as f32, 0.),
                ((width - 1) as f32, (height - 1) as f32),
                (0., (height - 1) as f32),
            ];
            let transform = Projection::from_control_points(quad, target)
                .context("degenerate detected quad")?;
            let mut crop = RgbImage::new(width, height);
            warp_into(
                image,
                &transform,
                Interpolation::Bilinear,
                Rgb([0, 0, 0]),
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
                .context("rotation projection failed")?;
                inverse = rotation.and_then(inverse);
            }
            let rec_width = (48. * crop.width() as f32 / crop.height() as f32).ceil() as u32;
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
            ensure!(shape.len() == 3, "recognizer output shape mismatch");
            let steps = shape[1] as usize;
            let classes = shape[2] as usize;
            ensure!(
                classes == self.dictionary.len(),
                "dictionary and recognition output differ: {} versus {}",
                self.dictionary.len(),
                classes
            );
            let tokens = ctc(data, steps, classes);
            let mut text = Sensitive(String::new());
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
                text.0.push_str(value);
                chars.push(json!({"text":value,"vertices":points.map(|p|[p.0,p.1]),"ctc_start":start,"ctc_end":end}));
            }
            if text.0.is_empty() {
                continue;
            }
            let vertices = quad.map(|p| [p.0, p.1]);
            lines.push(json!({"text":text.0,"vertices":vertices,"words":[{"text":text.0,"vertices":vertices}],"chars":chars,"ctc_steps":steps,"rec_input_width":padded_width,"rec_content_width":rec_width}));
        }
        Ok(Sensitive(
            json!({"image_width":image.width(),"image_height":image.height(),"paragraphs":[{"lines":lines}],"url":"synthetic-corpus"}),
        ))
    }
}
fn main() -> Result<()> {
    std::panic::set_hook(Box::new(|_| eprintln!("OCR runner panic [redacted]")));
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    ensure!(
        args.len() >= 3,
        "usage: s1b-ppocr-runner MODEL_DIR CORPUS_DIR OUTPUT_DIR [VARIANT=mobile] [THREADS=4] [MAX_SIDE=960] [LIMIT=all]"
    );
    let model = PathBuf::from(&args[0]);
    let corpus = PathBuf::from(&args[1]);
    let out = PathBuf::from(&args[2]);
    let variant = args
        .get(3)
        .map(|a| a.to_string_lossy().into_owned())
        .unwrap_or("mobile".into());
    ensure!(
        variant == "mobile" || variant == "server",
        "unsupported variant"
    );
    let number = |i: usize, default: usize| -> Result<usize> {
        Ok(match args.get(i) {
            Some(a) => a.to_string_lossy().parse()?,
            None => default,
        })
    };
    let threads = number(4, 4)?;
    let max_side = number(5, 960)? as u32;
    let limit = number(6, usize::MAX)?;
    let dictionary = fs::read_to_string(model.join(format!("{variant}_dict.txt")))?;
    let mut entries = vec![String::new()];
    entries.extend(dictionary.lines().map(str::to_owned));
    entries.push(" ".into());
    let mut engine = Engine {
        det: session(
            &model.join(format!("{variant}_det_inference.onnx")),
            threads,
        )?,
        rec: session(
            &model.join(format!("{variant}_rec_inference.onnx")),
            threads,
        )?,
        dictionary: entries,
        max_side,
    };
    let manifest: Manifest = serde_json::from_slice(&fs::read(corpus.join("manifest.json"))?)?;
    fs::create_dir_all(&out)?;
    let mut timings = Vec::new();
    for (index, entry) in manifest.images.iter().take(limit).enumerate() {
        ensure!(
            !entry.id.is_empty()
                && !entry.id.contains("..")
                && entry
                    .id
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || "-._".contains(c)),
            "invalid synthetic ID"
        );
        let image = image::open(corpus.join(format!("{}.png", entry.id)))?.into_rgb8();
        if index == 0 {
            engine.run(&image)?;
        }
        let start = Instant::now();
        let result = engine.run(&image)?;
        let seconds = start.elapsed().as_secs_f64();
        fs::write(
            out.join(format!("{}.ocr.json", entry.id)),
            serde_json::to_vec(&result.0)?,
        )?;
        timings.push(json!({"id":entry.id,"seconds":seconds,"lines":result.0["paragraphs"][0]["lines"].as_array().map_or(0,Vec::len),"width":image.width(),"height":image.height()}));
        fs::write(out.join("warm.json"), serde_json::to_vec_pretty(&timings)?)?;
        eprintln!(
            "Measured synthetic image {}/{} ({seconds:.3}s)",
            index + 1,
            manifest.images.len().min(limit)
        );
    }
    fs::write(
        out.join("runner-config.json"),
        serde_json::to_vec_pretty(
            &json!({"threads":threads,"inter_threads":1,"execution_provider":"CPU","variant":variant,"max_side":max_side,"det_threshold":0.3,"box_threshold":0.6,"unclip_ratio":1.5,"rec_height":48,"ort_crate":"2.0.0-rc.10","warmup":"first image once, excluded","image_decode":"excluded","ocr_pre_post":"included"}),
        )?,
    )?;
    Ok(())
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
        let s = Sensitive("synthetic-value");
        assert_eq!(format!("{s:?} {s}"), "[redacted] [redacted]");
    }
}
