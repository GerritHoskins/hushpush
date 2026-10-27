//! Read-only evaluation of the product image pipeline against the frozen S1 corpus.

use std::{
    collections::BTreeSet,
    error::Error,
    fs,
    path::{Path, PathBuf},
    process::Command,
    time::Instant,
};

use leaklens_core::{
    Sensitive,
    detect::Category,
    ocr::ImagePipeline,
    report::{Finding, Location},
    source::Origin,
};
use serde::{Deserialize, Deserializer};
use sha2::{Digest, Sha256};

#[derive(Deserialize)]
struct Manifest {
    images: Vec<ManifestImage>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ManifestImage {
    id: String,
    scale: f64,
    sha256: String,
    truth_sha256: String,
}

#[derive(Deserialize)]
struct Truth {
    plants: Vec<Plant>,
}

#[derive(Deserialize)]
struct Plant {
    category: String,
    #[serde(deserialize_with = "sensitive_string")]
    value: Sensitive<String>,
    #[serde(rename = "glyphBoxes")]
    glyph_boxes: Vec<Rect>,
}

fn sensitive_string<'de, D: Deserializer<'de>>(d: D) -> Result<Sensitive<String>, D::Error> {
    String::deserialize(d).map(Sensitive::new)
}

#[derive(Clone, Copy, Deserialize)]
struct Rect {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

struct Sample {
    scale: f64,
    bytes: Vec<u8>,
    truth: Truth,
}

#[derive(Default)]
struct Metric {
    planted: usize,
    found: usize,
    false_findings: usize,
    false_lookalikes: usize,
    coverage: Vec<f64>,
}

impl Metric {
    fn recall(&self) -> f64 {
        self.found as f64 / self.planted.max(1) as f64
    }
}

struct ScaleResult {
    scale: f64,
    metrics: [Metric; 2],
    seconds: Vec<f64>,
    skipped: usize,
}

struct Evaluation {
    scales: Vec<ScaleResult>,
}

impl Evaluation {
    fn passes(&self) -> bool {
        self.scales.iter().all(|s| {
            s.skipped == 0
                && s.metrics.iter().all(|m| {
                    (s.scale == 1.5 || m.recall() >= if s.scale == 1.0 { 0.85 } else { 0.95 })
                        && !m.coverage.is_empty()
                        && m.coverage.iter().all(|&c| c >= 0.98)
                })
                && (s.scale != 1.0 || s.seconds.iter().all(|&t| t <= 2.5))
        })
    }

    fn speed_passes(&self) -> bool {
        let s = &self.scales[0];
        s.skipped == 0 && s.seconds.iter().all(|&t| t <= 2.5)
    }

    fn one_x_found(&self) -> usize {
        self.scales[0].metrics.iter().map(|m| m.found).sum()
    }
}

pub fn run(args: &[String]) -> Result<(), Box<dyn Error>> {
    let models = match args {
        [] => std::env::var_os("LEAKLENS_MODELS").map(PathBuf::from),
        [flag, path] if flag == "--models" => Some(PathBuf::from(path)),
        _ => return Err("usage: cargo xtask eval-ocr [--models DIR]".into()),
    };
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or("workspace root unavailable")?;
    let samples = load_samples(&root.join("spikes/s1-ocr/out"))?;
    println!(
        "Product OCR evaluation: 60 frozen synthetic images; email + card only (phone detector unavailable)."
    );
    println!(
        "Reserved domains included. Exact canonical values, one-to-one geometric assignment; no scorer-driven text repair."
    );
    println!(
        "CPU: {}; OS: {}; logical CPUs: {}; ORT CPU threads: intra=4 inter=1.",
        cpu_name(),
        std::env::consts::OS,
        std::thread::available_parallelism().map_or(0, usize::from)
    );
    println!(
        "Warm wall times include product decode, optional Lanczos3 resize, OCR and detection; model warmup excluded. One sample per image; no laptop claim."
    );
    let models = models.or_else(|| Some(root.join("models")));
    let baseline = evaluate(&samples, models.clone(), false);
    let upscaled = evaluate(&samples, models, true);
    let keep_upscale = upscaled.one_x_found() > baseline.one_x_found() && upscaled.speed_passes();
    println!(
        "Small-image 2x upscale decision: {} (1x found baseline={} upscale={}; upscale speed gate={}).",
        if keep_upscale { "KEEP" } else { "REJECT" },
        baseline.one_x_found(),
        upscaled.one_x_found(),
        verdict(upscaled.speed_passes())
    );
    let selected = if keep_upscale { &upscaled } else { &baseline };
    println!(
        "Selected product configuration gates: {}",
        verdict(selected.passes())
    );
    if selected.passes() {
        Ok(())
    } else {
        Err("OCR acceptance gates failed; see aggregate metrics above".into())
    }
}

fn load_samples(dir: &Path) -> Result<Vec<Sample>, Box<dyn Error>> {
    let manifest: Manifest = serde_json::from_slice(&fs::read(dir.join("manifest.json"))?)?;
    if manifest.images.len() != 60 {
        return Err("expected exactly 60 frozen corpus images".into());
    }
    let mut ids = BTreeSet::new();
    let mut samples = Vec::new();
    for image in manifest.images {
        if image.id.is_empty()
            || !image
                .id
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '.')
            || image.id.contains("..")
            || !ids.insert(image.id.clone())
        {
            return Err("invalid or repeated corpus image identifier".into());
        }
        let bytes = fs::read(dir.join(format!("{}.png", image.id)))?;
        let truth_bytes = fs::read(dir.join(format!("{}.json", image.id)))?;
        if format!("{:x}", Sha256::digest(&bytes)) != image.sha256
            || format!("{:x}", Sha256::digest(compact_json(&truth_bytes))) != image.truth_sha256
        {
            return Err("frozen corpus SHA-256 mismatch".into());
        }
        // Avoid echoing a serde error's input excerpt if a truth artifact is malformed.
        let truth =
            serde_json::from_slice(&truth_bytes).map_err(|_| "invalid frozen truth schema")?;
        samples.push(Sample {
            scale: image.scale,
            bytes,
            truth,
        });
    }
    for scale in [1.0, 1.5, 2.0] {
        if samples.iter().filter(|s| s.scale == scale).count() != 20 {
            return Err("expected 20 corpus images at each scale".into());
        }
    }
    println!("Corpus verified: all 60 PNG and 60 truth SHA-256 hashes match the manifest.");
    Ok(samples)
}

// The generator hashes JSON.stringify(truth), then writes pretty JSON. Preserve
// its original number spelling, property order and string escaping for hashing.
fn compact_json(bytes: &[u8]) -> Vec<u8> {
    let mut compact = Vec::with_capacity(bytes.len());
    let mut in_string = false;
    let mut escaped = false;
    for &byte in bytes {
        if in_string {
            compact.push(byte);
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                in_string = false;
            }
        } else if byte == b'"' {
            in_string = true;
            compact.push(byte);
        } else if !matches!(byte, b' ' | b'\t' | b'\n' | b'\r') {
            compact.push(byte);
        }
    }
    compact
}

fn evaluate(samples: &[Sample], models: Option<PathBuf>, upscale: bool) -> Evaluation {
    let pipeline = ImagePipeline::new(models, upscale);
    let mode = if upscale {
        "upscale-small-2x"
    } else {
        "baseline"
    };
    let warmup = pipeline.scan(
        &samples[0].bytes,
        "synthetic-warmup.png".into(),
        Origin::default(),
        true,
    );
    if let Err(reason) = warmup {
        println!("{mode} warmup skipped: {}", reason.as_str());
    }
    let mut result = Evaluation { scales: Vec::new() };
    for scale in [1.0, 1.5, 2.0] {
        let mut row = ScaleResult {
            scale,
            metrics: Default::default(),
            seconds: Vec::new(),
            skipped: 0,
        };
        for sample in samples.iter().filter(|s| s.scale == scale) {
            let start = Instant::now();
            let findings = pipeline.scan(
                &sample.bytes,
                "synthetic-evaluation.png".into(),
                Origin::default(),
                true,
            );
            let seconds = start.elapsed().as_secs_f64();
            let findings = match findings {
                Ok(findings) => {
                    row.seconds.push(seconds);
                    findings
                }
                Err(_) => {
                    row.skipped += 1;
                    Vec::new()
                }
            };
            score(&sample.truth, &findings, &mut row.metrics);
        }
        print_row(mode, &row);
        result.scales.push(row);
    }
    println!("{mode} all acceptance gates: {}", verdict(result.passes()));
    result
}

fn category_index(category: Category) -> Option<usize> {
    match category {
        Category::Email => Some(0),
        Category::PaymentCard => Some(1),
        _ => None,
    }
}

fn canonical(category: &str, value: &str) -> Sensitive<String> {
    Sensitive::new(if category == "email" {
        value
            .chars()
            .filter(|c| !c.is_whitespace())
            .flat_map(char::to_lowercase)
            .collect()
    } else {
        value.chars().filter(char::is_ascii_digit).collect()
    })
}

fn finding_rect(finding: &Finding) -> Option<Rect> {
    if let Location::Bbox { x, y, w, h } = finding.location {
        Some(Rect {
            x: x as f64,
            y: y as f64,
            width: w as f64,
            height: h as f64,
        })
    } else {
        None
    }
}

fn score(truth: &Truth, findings: &[Finding], metrics: &mut [Metric; 2]) {
    let mut used = BTreeSet::new();
    for plant in &truth.plants {
        let index = match plant.category.as_str() {
            "email" => 0,
            "card" => 1,
            _ => continue,
        };
        let metric = &mut metrics[index];
        metric.planted += 1;
        let expected = canonical(&plant.category, plant.value.expose());
        let candidate = findings
            .iter()
            .enumerate()
            .filter_map(|(i, f)| {
                if used.contains(&i)
                    || category_index(f.category) != Some(index)
                    || canonical(&plant.category, f.value.expose()).expose() != expected.expose()
                {
                    return None;
                }
                let area = coverage(&plant.glyph_boxes, finding_rect(f)?);
                (area > 0.0).then_some((i, area))
            })
            .max_by(|a, b| a.1.total_cmp(&b.1));
        if let Some((i, area)) = candidate {
            used.insert(i);
            metric.found += 1;
            metric.coverage.push(area);
        }
    }
    for finding in findings
        .iter()
        .enumerate()
        .filter(|(i, _)| !used.contains(i))
        .map(|(_, finding)| finding)
    {
        let Some(index) = category_index(finding.category) else {
            continue;
        };
        let metric = &mut metrics[index];
        metric.false_findings += 1;
        if finding_rect(finding).is_some_and(|rect| {
            truth
                .plants
                .iter()
                .any(|p| p.category == "benign" && coverage(&p.glyph_boxes, rect) > 0.0)
        }) {
            metric.false_lookalikes += 1;
        }
    }
}

fn print_row(mode: &str, row: &ScaleResult) {
    for (name, metric) in ["email", "card"].iter().zip(&row.metrics) {
        let minimum = metric
            .coverage
            .iter()
            .copied()
            .reduce(f64::min)
            .unwrap_or(0.0);
        println!(
            "{mode} scale={} {name}: found={}/{} recall={:.2}% false-lookalikes={} false-total={} coverage+8 mean={:.2}% worst={:.2}% >=98%={}/{}",
            row.scale,
            metric.found,
            metric.planted,
            100.0 * metric.recall(),
            metric.false_lookalikes,
            metric.false_findings,
            100.0 * mean(&metric.coverage),
            100.0 * minimum,
            metric.coverage.iter().filter(|&&c| c >= 0.98).count(),
            metric.coverage.len()
        );
    }
    println!(
        "{mode} scale={} warm images={} skipped={} mean={:.3}s max={:.3}s",
        row.scale,
        row.seconds.len(),
        row.skipped,
        mean(&row.seconds),
        row.seconds.iter().copied().reduce(f64::max).unwrap_or(0.0)
    );
}

fn verdict(pass: bool) -> &'static str {
    if pass { "PASS" } else { "FAIL" }
}
fn mean(values: &[f64]) -> f64 {
    values.iter().sum::<f64>() / values.len().max(1) as f64
}

// Product boxes already contain the requested 8 px padding; do not pad twice.
fn coverage(glyphs: &[Rect], rect: Rect) -> f64 {
    let total = union_area(glyphs);
    if total <= 0.0 {
        return 0.0;
    }
    let clipped: Vec<_> = glyphs
        .iter()
        .filter_map(|g| {
            let x = g.x.max(rect.x);
            let y = g.y.max(rect.y);
            let right = (g.x + g.width).min(rect.x + rect.width);
            let bottom = (g.y + g.height).min(rect.y + rect.height);
            (right > x && bottom > y).then_some(Rect {
                x,
                y,
                width: right - x,
                height: bottom - y,
            })
        })
        .collect();
    (union_area(&clipped) / total).clamp(0.0, 1.0)
}

fn union_area(rects: &[Rect]) -> f64 {
    let mut xs: Vec<_> = rects.iter().flat_map(|r| [r.x, r.x + r.width]).collect();
    xs.sort_by(f64::total_cmp);
    xs.dedup();
    xs.windows(2)
        .map(|xs| {
            let mut intervals: Vec<_> = rects
                .iter()
                .filter(|r| r.x < xs[1] && r.x + r.width > xs[0])
                .map(|r| (r.y, r.y + r.height))
                .collect();
            intervals.sort_by(|a, b| a.0.total_cmp(&b.0));
            let mut end = f64::NEG_INFINITY;
            let mut length = 0.0;
            for (a, b) in intervals {
                length += (b - a.max(end)).max(0.0);
                end = end.max(b);
            }
            (xs[1] - xs[0]) * length
        })
        .sum()
}

fn cpu_name() -> String {
    let result = if cfg!(target_os = "windows") {
        Command::new("powershell")
            .args([
                "-NoProfile",
                "-Command",
                "(Get-CimInstance Win32_Processor).Name",
            ])
            .output()
    } else if cfg!(target_os = "macos") {
        Command::new("sysctl")
            .args(["-n", "machdep.cpu.brand_string"])
            .output()
    } else {
        Command::new("lscpu").output()
    };
    result
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| {
            let text = String::from_utf8(o.stdout).ok()?;
            if cfg!(target_os = "linux") {
                text.lines()
                    .find(|l| l.starts_with("Model name:"))
                    .map(|l| l.trim_start_matches("Model name:").trim().to_owned())
            } else {
                Some(text.trim().to_owned())
            }
        })
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| format!("unknown ({})", std::env::consts::ARCH))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truth_hash_compaction_preserves_strings_and_escapes() {
        let pretty = br#"{
            "space": "a b", "escaped": "a\" b", "slash": "a\\", 
            "number": 0.03125, "unicode": "\u0041"
        }
        "#;
        let compact = br#"{"space":"a b","escaped":"a\" b","slash":"a\\","number":0.03125,"unicode":"\u0041"}"#;
        assert_eq!(compact_json(pretty), compact);
    }

    #[test]
    fn coverage_counts_overlapping_glyph_ranges_once() {
        let glyphs = [
            Rect {
                x: 0.0,
                y: 0.0,
                width: 10.0,
                height: 10.0,
            },
            Rect {
                x: 5.0,
                y: 0.0,
                width: 10.0,
                height: 10.0,
            },
        ];
        let redaction = Rect {
            x: 0.0,
            y: 0.0,
            width: 7.5,
            height: 10.0,
        };
        assert_eq!(union_area(&glyphs), 150.0);
        assert_eq!(coverage(&glyphs, redaction), 0.5);
    }
}
