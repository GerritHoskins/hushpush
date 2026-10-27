//! Independent corpus scoring through the release CLI's public JSON contract.
use hushpush_core::{ocr::ImagePipeline, source::Origin};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fs,
    io::Write,
    path::{Component, Path, PathBuf},
    process::{Command, Output},
    time::Instant,
};

type Result<T> = std::result::Result<T, Box<dyn Error>>;

#[derive(Deserialize)]
struct Manifest {
    version: u32,
    seed: u64,
    files: Vec<Fixture>,
}
#[derive(Deserialize)]
struct Fixture {
    path: String,
    sha256: Option<String>,
    context: String,
    kind: String,
    template: Option<String>,
    scale: Option<f64>,
    expected_coverage: String,
    #[serde(default)]
    expected_reason: Option<String>,
    labels: Vec<Label>,
}
#[derive(Deserialize)]
struct Label {
    category: String,
    severity: String,
    core: Region,
    extent: Region,
    off_spec: Option<String>,
}
#[derive(Deserialize)]
struct Region {
    start: Option<usize>,
    end: Option<usize>,
    #[serde(default)]
    boxes: Vec<[f64; 4]>,
}
#[derive(Default, Serialize)]
struct Counts {
    tp: usize,
    fp: usize,
    fn_: usize,
    precision: f64,
    recall: f64,
}
impl Counts {
    fn finish(&mut self) {
        self.precision = ratio(self.tp, self.tp + self.fp);
        self.recall = ratio(self.tp, self.tp + self.fn_);
    }
}
fn ratio(a: usize, b: usize) -> f64 {
    if b == 0 { 1.0 } else { a as f64 / b as f64 }
}
#[derive(Serialize)]
struct Gate {
    name: String,
    value: f64,
    target: String,
    pass: bool,
}
#[derive(Serialize)]
struct Coverage {
    path: String,
    expected: String,
    actual: String,
    expected_reason: Option<String>,
    actual_reason: Option<String>,
    pass: bool,
}
#[derive(Serialize)]
struct Miss {
    path: String,
    category: String,
    severity: String,
    start: Option<usize>,
    end: Option<usize>,
    best_overlap: f64,
    cause: String,
}
#[derive(Serialize)]
struct Evaluation {
    seed: u64,
    fixture_sha256: String,
    metrics: BTreeMap<String, Counts>,
    misses: Vec<Miss>,
    spill_bytes: usize,
    spill_predictions: usize,
    negative_false_findings: usize,
    coverage: Vec<Coverage>,
    localization_mean: f64,
    localization_min: f64,
    residual_findings: usize,
    unchanged_inputs: bool,
    redaction_exit: Option<i32>,
    redaction_missing_outputs: Vec<String>,
    redaction_missing_supported: usize,
    supported_fixtures: usize,
    text_seconds: f64,
    image_mean_seconds: f64,
    image_max_seconds: f64,
    image_timing_failures: usize,
    cpu: String,
    gates: Vec<Gate>,
    regression_metrics: BTreeMap<String, f64>,
}
#[derive(Serialize, Deserialize)]
struct Baseline {
    version: u32,
    fixture_sha256: String,
    metrics: BTreeMap<String, f64>,
    test_manifest_sha256: String,
}
pub fn run(args: &[String]) -> Result<()> {
    let mut split = "dev";
    let mut ci = false;
    let mut write_baseline = false;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--split" => split = iter.next().ok_or("--split needs dev or test")?,
            "--ci" => ci = true,
            "--write-baseline" => write_baseline = true,
            _ => {
                return Err(
                    "usage: cargo xtask eval [--split dev|test] [--ci|--write-baseline]".into(),
                );
            }
        }
    }
    if !matches!(split, "dev" | "test")
        || (ci && write_baseline)
        || (split == "test" && (ci || write_baseline))
    {
        return Err("CI and baseline writes require dev; --ci cannot write a baseline".into());
    }
    if split == "test" {
        println!("Reminder: sealed test split is for milestone checks, not tuning.");
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or("workspace root unavailable")?;
    let base_corpus = root.join("corpus");
    let corpus = if split == "test" {
        base_corpus.join("test")
    } else {
        base_corpus.clone()
    };
    let manifest: Manifest = serde_json::from_slice(&fs::read(corpus.join("labels.json"))?)
        .map_err(|_| "invalid corpus manifest")?;
    if manifest.version != 2
        || manifest.seed != if split == "test" { 20261028 } else { 20261027 }
        || manifest.files.is_empty()
    {
        return Err("invalid corpus identity".into());
    }
    if split == "test" {
        let baseline: Baseline =
            serde_json::from_slice(&fs::read(base_corpus.join("baseline.json"))?)?;
        let hash = format!(
            "{:x}",
            Sha256::digest(fs::read(corpus.join("labels.json"))?)
        );
        if baseline.version != 2 || baseline.test_manifest_sha256 != hash {
            return Err("sealed test manifest differs from the recorded baseline".into());
        }
    }
    let target = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("target"));
    let target = if target.is_absolute() {
        target
    } else {
        root.join(target)
    };
    let cli = target
        .join("release")
        .join(format!("hushpush{}", std::env::consts::EXE_SUFFIX));
    if !cli.is_file() {
        return Err("build the release CLI before evaluation: cargo build --release".into());
    }
    let scratch = tempfile::Builder::new()
        .prefix("hushpush-eval-")
        .tempdir()?;
    let staged = scratch.path().join("corpus");
    fs::create_dir(&staged)?;
    let mut seen = BTreeSet::new();
    let mut before = BTreeMap::new();
    let mut digest = Sha256::new();
    digest.update(fs::read(corpus.join("labels.json"))?);
    for file in &manifest.files {
        validate_path(&file.path)?;
        if !seen.insert(&file.path) {
            return Err("duplicate fixture path".into());
        }
        let source = corpus.join(&file.path);
        let dest = staged.join(&file.path);
        fs::create_dir_all(dest.parent().ok_or("invalid fixture path")?)?;
        let bytes = fixture_bytes(&source)?;
        if file
            .sha256
            .as_ref()
            .is_some_and(|expected| *expected != format!("{:x}", Sha256::digest(&bytes)))
        {
            return Err(
                "fixture SHA-256 mismatch; corpus is frozen, do not repair it from scores".into(),
            );
        }
        digest.update(file.path.as_bytes());
        digest.update(&bytes);
        before.insert(file.path.clone(), format!("{:x}", Sha256::digest(&bytes)));
        copy_fixture(&source, &dest)?;
        validate_labels(file, &bytes)?;
    }
    digest.update(fs::read(corpus.join("patterns.toml"))?);
    let fixture_sha256 = format!("{:x}", digest.finalize());
    let patterns = corpus.join("patterns.toml").canonicalize()?;
    let models = std::env::var_os("HUSHPUSH_MODELS")
        .map(PathBuf::from)
        .or_else(|| Some(root.join("models")));
    let models = models.map(|p| p.canonicalize()).transpose()?;
    println!(
        "Evaluating {} independent fixtures through the release CLI",
        manifest.files.len()
    );
    let runner = Runner {
        cli,
        cwd: scratch.path().to_path_buf(),
        patterns,
        models: models.clone(),
    };
    let report = runner.scan(&staged, None)?;
    let mut result = score(&manifest, &staged, &report)?;
    result.fixture_sha256 = fixture_sha256;
    let out = scratch.path().join("redacted");
    let redacted = runner.command(&["redact"], &staged, Some(&out), None)?;
    result.redaction_exit = redacted.status.code();
    if !matches!(redacted.status.code(), Some(0 | 1 | 3 | 4)) {
        return Err(format!(
            "redaction command failed operationally with exit {:?} (output withheld)",
            redacted.status.code()
        )
        .into());
    }
    result.residual_findings = runner.scan(&out, None)?["findings"]
        .as_array()
        .ok_or("invalid rescan report")?
        .len();
    for file in &manifest.files {
        if !out.join(&file.path).exists() {
            result.redaction_missing_outputs.push(file.path.clone());
            if file.kind != "edge" {
                result.redaction_missing_supported += 1;
            }
        }
    }
    result.unchanged_inputs = manifest.files.iter().all(|f| {
        [&corpus, &staged].iter().all(|dir| {
            fixture_bytes(&dir.join(&f.path))
                .is_ok_and(|b| before[&f.path] == format!("{:x}", Sha256::digest(b)))
        })
    });
    println!("Measuring generated 50 MB text and warm images");
    let perf = scratch.path().join("throughput");
    fs::create_dir(&perf)?;
    let mut log = fs::File::create(perf.join("synthetic.log"))?;
    let line = b"2026-10-27T12:34:56Z INFO build completed cache=warm version=1.2.3 hash=abcdef1234567890\n";
    let size = 50_000_000;
    let chunk = line.repeat(8192);
    let mut written = 0;
    while written < size {
        let n = chunk.len().min(size - written);
        log.write_all(&chunk[..n])?;
        written += n;
    }
    drop(log);
    let config = scratch.path().join("performance.toml");
    fs::write(&config, "version = 1\n[limits]\nmax_text_mib = 64\n")?;
    let start = Instant::now();
    let perf_report = runner.scan(&perf, Some(&config))?;
    result.text_seconds = start.elapsed().as_secs_f64();
    if perf_report["coverage"]["scanned"].as_u64() != Some(1)
        || !perf_report["coverage"]["skipped"]
            .as_array()
            .ok_or("invalid throughput report")?
            .is_empty()
    {
        return Err("throughput input was not fully scanned".into());
    }
    let pipeline = ImagePipeline::new(models, true);
    let images: Vec<_> = manifest
        .files
        .iter()
        .filter(|f| f.kind == "image")
        .collect();
    let mut timings = Vec::new();
    if let Some(first) = images.first() {
        let _ = pipeline.scan(
            &fs::read(staged.join(&first.path))?,
            "warmup.png".into(),
            Origin::default(),
            true,
        );
    }
    for image in images {
        let bytes = fs::read(staged.join(&image.path))?;
        let start = Instant::now();
        let scan = pipeline.scan(&bytes, "timing.png".into(), Origin::default(), true);
        timings.push(start.elapsed().as_secs_f64());
        if scan.is_err() {
            result.image_timing_failures += 1;
        }
    }
    result.image_mean_seconds = timings.iter().sum::<f64>() / timings.len().max(1) as f64;
    result.image_max_seconds = timings.into_iter().fold(0.0, f64::max);
    result.cpu = cpu_description();
    finish(&mut result);
    fs::create_dir_all(&target)?;
    fs::write(
        target.join(if split == "dev" {
            "eval.json"
        } else {
            "eval-test.json"
        }),
        serde_json::to_vec_pretty(&result)?,
    )?;
    let markdown = render(&result);
    fs::write(corpus.join("RESULTS.md"), &markdown)?;
    println!("{markdown}");
    let baseline_path = base_corpus.join("baseline.json");
    let test_manifest_sha256 = format!(
        "{:x}",
        Sha256::digest(fs::read(base_corpus.join("test/labels.json"))?)
    );
    if write_baseline {
        fs::write(
            &baseline_path,
            serde_json::to_vec_pretty(&Baseline {
                version: 2,
                test_manifest_sha256: test_manifest_sha256.clone(),
                fixture_sha256: result.fixture_sha256.clone(),
                metrics: result.regression_metrics.clone(),
            })?,
        )?;
    }
    if ci {
        let baseline: Baseline = serde_json::from_slice(&fs::read(baseline_path)?)
            .map_err(|_| "invalid evaluation baseline")?;
        if baseline.version != 2
            || baseline.fixture_sha256 != result.fixture_sha256
            || baseline.test_manifest_sha256 != test_manifest_sha256
        {
            return Err(
                "baseline corpus identity mismatch; review and regenerate deliberately".into(),
            );
        }
        let regressions = regressions(&baseline.metrics, &result.regression_metrics);
        if !regressions.is_empty() {
            return Err(format!(
                "metric regression exceeding one percentage point: {}",
                regressions.join(", ")
            )
            .into());
        }
    }
    Ok(())
}

fn validate_path(path: &str) -> Result<()> {
    if path.is_empty()
        || Path::new(path)
            .components()
            .any(|c| !matches!(c, Component::Normal(_)))
    {
        return Err("fixture paths must be relative normal components".into());
    }
    Ok(())
}
fn fixture_bytes(path: &Path) -> Result<Vec<u8>> {
    if fs::symlink_metadata(path)?.file_type().is_symlink() {
        return Ok(fs::read_link(path)?.to_string_lossy().as_bytes().to_vec());
    }
    Ok(fs::read(path)?)
}
fn copy_fixture(source: &Path, dest: &Path) -> Result<()> {
    if fs::symlink_metadata(source)?.file_type().is_symlink() {
        #[cfg(unix)]
        std::os::unix::fs::symlink(fs::read_link(source)?, dest)?;
        #[cfg(windows)]
        std::os::windows::fs::symlink_file(fs::read_link(source)?, dest)?;
    } else {
        fs::copy(source, dest)?;
    }
    Ok(())
}
fn validate_labels(file: &Fixture, bytes: &[u8]) -> Result<()> {
    for l in &file.labels {
        if !matches!(l.severity.as_str(), "block" | "warn") {
            return Err("invalid label severity".into());
        }
        for region in [&l.core, &l.extent] {
            if file.kind == "image" {
                if region.boxes.is_empty()
                    || region.boxes.iter().any(|b| {
                        b.iter().any(|v| !v.is_finite() || *v < 0.0) || b[2] == 0.0 || b[3] == 0.0
                    })
                {
                    return Err("invalid glyph boxes".into());
                }
            } else if !matches!((region.start, region.end), (Some(s), Some(e)) if s < e && e <= bytes.len() && std::str::from_utf8(&bytes[s..e]).is_ok())
            {
                return Err("invalid planted region".into());
            }
        }
        if file.kind != "image" && (l.extent.start > l.core.start || l.extent.end < l.core.end) {
            return Err("core outside extent".into());
        }
        if l.off_spec.as_ref().is_some_and(|g| {
            !matches!(
                g.as_str(),
                "phone/national-no-country-code" | "clipboard/undocumented-key"
            )
        }) {
            return Err("unknown off-spec group".into());
        }
    }
    Ok(())
}
struct Runner {
    cli: PathBuf,
    cwd: PathBuf,
    patterns: PathBuf,
    models: Option<PathBuf>,
}
impl Runner {
    fn command(
        &self,
        prefix: &[&str],
        path: &Path,
        out: Option<&Path>,
        config: Option<&Path>,
    ) -> Result<Output> {
        let mut c = Command::new(&self.cli);
        c.current_dir(&self.cwd)
            .args(prefix)
            .arg(path)
            .args(["--include-reserved", "--no-allow"])
            .env("HUSHPUSH_PATTERNS", &self.patterns);
        if prefix.first() == Some(&"scan") {
            c.args(["--json", "-"]);
        }
        for key in [
            "GIT_DIR",
            "GIT_WORK_TREE",
            "GIT_INDEX_FILE",
            "GIT_COMMON_DIR",
        ] {
            c.env_remove(key);
        }
        if let Some(models) = &self.models {
            c.arg("--models").arg(models);
        }
        if let Some(out) = out {
            c.arg("--out").arg(out);
        }
        if let Some(config) = config {
            c.arg("--config").arg(config);
        }
        Ok(c.output()?)
    }
    fn scan(&self, path: &Path, config: Option<&Path>) -> Result<Value> {
        let output = self.command(&["scan", "dir"], path, None, config)?;
        if !matches!(output.status.code(), Some(0 | 1 | 3)) {
            return Err("release CLI scan failed operationally (output withheld)".into());
        }
        let value: Value = serde_json::from_slice(&output.stdout)
            .map_err(|_| "invalid CLI JSON report (output withheld)")?;
        if value["schema"] != "hushpush.report/v1" {
            return Err("unexpected CLI JSON schema".into());
        }
        Ok(value)
    }
}

fn groups(file: &Fixture, category: &str, severity: &str) -> Vec<String> {
    let mut v = vec![
        "overall".into(),
        format!("category/{category}"),
        format!("severity/{severity}"),
        format!("category/{category}/severity/{severity}"),
        format!("context/{}", file.context),
        file.kind.clone(),
        format!("{}/category/{category}", file.kind),
    ];
    if let Some(scale) = file.scale {
        v.push(format!("image/scale/{scale}"));
        v.push(format!("image/scale/{scale}/category/{category}"));
    }
    if let Some(template) = &file.template {
        v.push(format!("image/template/{template}"));
        if let Some(scale) = file.scale {
            v.push(format!("image/template/{template}/scale/{scale}"));
        }
    }
    v
}
fn text_span(bytes: &[u8], location: &Value) -> Option<(usize, usize)> {
    let text = std::str::from_utf8(bytes).ok()?;
    let line = location["line"].as_u64()? as usize;
    let column = location["col_start"].as_u64()? as usize;
    let end_column = location["col_end"].as_u64()? as usize;
    let offset = if line == 1 {
        0
    } else {
        text.match_indices('\n')
            .nth(line.checked_sub(2)?)
            .map(|(i, _)| i + 1)?
    };
    let tail = &text[offset..];
    let char_byte = |n| {
        tail.char_indices()
            .nth(n)
            .map(|(i, _)| i)
            .or_else(|| (tail.chars().count() == n).then_some(tail.len()))
    };
    Some((
        offset + char_byte(column.checked_sub(1)?)?,
        offset + char_byte(end_column.checked_sub(1)?)?,
    ))
}
fn span_overlap(a: (usize, usize), b: (usize, usize)) -> f64 {
    a.1.min(b.1).saturating_sub(a.0.max(b.0)) as f64 / (a.1 - a.0).max(1) as f64
}
fn rect(location: &Value) -> Option<[f64; 4]> {
    Some([
        location["x"].as_f64()?,
        location["y"].as_f64()?,
        location["w"].as_f64()?,
        location["h"].as_f64()?,
    ])
}
fn area_union(rects: &[[f64; 4]]) -> f64 {
    let mut xs: Vec<_> = rects.iter().flat_map(|r| [r[0], r[0] + r[2]]).collect();
    xs.sort_by(f64::total_cmp);
    xs.dedup();
    xs.windows(2)
        .map(|x| {
            let mut intervals: Vec<_> = rects
                .iter()
                .filter(|r| r[0] < x[1] && r[0] + r[2] > x[0])
                .map(|r| (r[1], r[1] + r[3]))
                .collect();
            intervals.sort_by(|a, b| a.0.total_cmp(&b.0));
            let mut length = 0.0;
            let mut end = f64::NEG_INFINITY;
            for (a, b) in intervals {
                length += (b - a.max(end)).max(0.0);
                end = end.max(b);
            }
            (x[1] - x[0]) * length
        })
        .sum()
}
fn box_coverage(boxes: &[[f64; 4]], found: [f64; 4]) -> f64 {
    let clipped: Vec<_> = boxes
        .iter()
        .map(|b| {
            let x = b[0].max(found[0]);
            let y = b[1].max(found[1]);
            [
                x,
                y,
                (b[0] + b[2]).min(found[0] + found[2]) - x,
                (b[1] + b[3]).min(found[1] + found[3]) - y,
            ]
        })
        .filter(|b| b[2] > 0.0 && b[3] > 0.0)
        .collect();
    area_union(&clipped) / area_union(boxes).max(f64::EPSILON)
}
fn assign_match(
    label: usize,
    candidates: &[Vec<(usize, f64)>],
    owners: &mut [Option<usize>],
    visited: &mut [bool],
) -> bool {
    for &(prediction, _) in &candidates[label] {
        if visited[prediction] {
            continue;
        }
        visited[prediction] = true;
        if owners[prediction]
            .is_none_or(|previous| assign_match(previous, candidates, owners, visited))
        {
            owners[prediction] = Some(label);
            return true;
        }
    }
    false
}
fn score(manifest: &Manifest, staged: &Path, report: &Value) -> Result<Evaluation> {
    let findings = report["findings"].as_array().ok_or("invalid findings")?;
    if findings.iter().any(|f| {
        !manifest.files.iter().any(|file| {
            f["source"]["path"]
                .as_str()
                .is_some_and(|p| p.replace('\\', "/") == file.path.replace('\\', "/"))
        })
    }) {
        return Err("report contains an unlisted fixture finding".into());
    }
    let skipped = report["coverage"]["skipped"]
        .as_array()
        .ok_or("invalid coverage")?;
    let mut metrics = BTreeMap::<String, Counts>::new();
    let mut coverage = Vec::new();
    let mut negative_fp = 0;
    let mut spill_bytes = 0;
    let mut spill_predictions = 0;
    let mut localization = Vec::new();
    let mut misses = Vec::new();
    for file in &manifest.files {
        let bytes = fixture_bytes(&staged.join(&file.path))?;
        let predicted: Vec<_> = findings
            .iter()
            .filter(|f| {
                f["source"]["path"]
                    .as_str()
                    .is_some_and(|p| p.replace('\\', "/") == file.path.replace('\\', "/"))
            })
            .collect();
        if file.kind != "image" {
            for finding in &predicted {
                if let Some((start, end)) = text_span(&bytes, &finding["location"]) {
                    let mut intervals: Vec<_> = file
                        .labels
                        .iter()
                        .filter(|l| finding["category"] == l.category)
                        .filter_map(|l| Some((start.max(l.extent.start?), end.min(l.extent.end?))))
                        .filter(|(a, b)| a < b)
                        .collect();
                    intervals.sort_unstable();
                    let mut cursor = start;
                    let mut inside = 0;
                    for (a, b) in intervals {
                        inside += b.saturating_sub(a.max(cursor));
                        cursor = cursor.max(b);
                    }
                    let spill = (end - start).saturating_sub(inside);
                    spill_bytes += spill;
                    spill_predictions += usize::from(spill > 0);
                }
            }
        }
        let mut used = BTreeSet::new();
        // Maximum bipartite matching prevents nearby plants from stealing unique predictions.
        let mut candidates: Vec<_> = file
            .labels
            .iter()
            .map(|l| {
                let mut options: Vec<_> = predicted
                    .iter()
                    .enumerate()
                    .filter_map(|(i, f)| {
                        if f["category"] != l.category || f["severity"] != l.severity {
                            return None;
                        }
                        let overlap = if file.kind == "image" {
                            box_coverage(&l.core.boxes, rect(&f["location"])?)
                        } else {
                            span_overlap(
                                (l.core.start?, l.core.end?),
                                text_span(&bytes, &f["location"])?,
                            )
                        };
                        (overlap + 1e-9 >= if file.kind == "image" { 0.98 } else { 0.90 })
                            .then_some((i, overlap))
                    })
                    .collect();
                options.sort_by(|a, b| b.1.total_cmp(&a.1));
                (l, options)
            })
            .collect();
        candidates.sort_by_key(|(_, o)| o.len());
        let options: Vec<_> = candidates
            .iter()
            .map(|(_, options)| options.clone())
            .collect();
        let mut owners = vec![None; predicted.len()];
        for label in 0..candidates.len() {
            assign_match(
                label,
                &options,
                &mut owners,
                &mut vec![false; predicted.len()],
            );
        }
        for (label_index, (label, options)) in candidates.into_iter().enumerate() {
            let matched = options
                .into_iter()
                .find(|(i, _)| owners[*i] == Some(label_index));
            if let Some((i, overlap)) = matched {
                used.insert(i);
                if file.kind == "image" {
                    localization.push(overlap);
                }
            } else {
                let best_overlap = predicted
                    .iter()
                    .filter(|f| f["category"] == label.category)
                    .filter_map(|f| {
                        if file.kind == "image" {
                            Some(box_coverage(&label.core.boxes, rect(&f["location"])?))
                        } else {
                            Some(span_overlap(
                                (label.core.start?, label.core.end?),
                                text_span(&bytes, &f["location"])?,
                            ))
                        }
                    })
                    .fold(0.0, f64::max);
                let cause = if best_overlap == 0.0 {
                    "no overlapping prediction of the labeled category"
                } else if best_overlap + 1e-9 < if file.kind == "image" { 0.98 } else { 0.90 } {
                    "overlap below scoring threshold"
                } else {
                    "severity mismatch or prediction already assigned to another plant"
                };
                misses.push(Miss {
                    path: file.path.clone(),
                    category: label.category.clone(),
                    severity: label.severity.clone(),
                    start: label.core.start,
                    end: label.core.end,
                    best_overlap,
                    cause: cause.into(),
                });
            }
            for g in label.off_spec.as_ref().map_or_else(
                || groups(file, &label.category, &label.severity),
                |g| vec![format!("off-spec/{g}")],
            ) {
                let count = metrics.entry(g).or_default();
                if matched.is_some() {
                    count.tp += 1;
                } else {
                    count.fn_ += 1;
                }
            }
        }
        for (i, f) in predicted.iter().enumerate() {
            if !used.contains(&i) {
                if let Some(label) = file.labels.iter().find(|l| {
                    l.off_spec.is_some()
                        && f["category"] == l.category
                        && text_span(&bytes, &f["location"]).is_some_and(|p| {
                            span_overlap(
                                (l.extent.start.unwrap_or(0), l.extent.end.unwrap_or(0)),
                                p,
                            ) > 0.0
                        })
                }) {
                    metrics
                        .entry(format!("off-spec/{}", label.off_spec.as_ref().unwrap()))
                        .or_default()
                        .fp += 1;
                    continue;
                }
                if file.labels.is_empty()
                    || !file.labels.iter().any(|l| {
                        if file.kind == "image" {
                            rect(&f["location"])
                                .is_some_and(|r| box_coverage(&l.core.boxes, r) > 0.0)
                        } else {
                            text_span(&bytes, &f["location"]).is_some_and(|s| {
                                span_overlap(
                                    (l.core.start.unwrap_or(0), l.core.end.unwrap_or(0)),
                                    s,
                                ) > 0.0
                            })
                        }
                    })
                {
                    negative_fp += 1;
                }
                for g in groups(
                    file,
                    f["category"].as_str().ok_or("missing category")?,
                    f["severity"].as_str().ok_or("missing severity")?,
                ) {
                    metrics.entry(g).or_default().fp += 1;
                }
            }
        }
        let skips: Vec<_> = skipped
            .iter()
            .filter(|s| {
                s["path"]
                    .as_str()
                    .is_some_and(|p| p.replace('\\', "/") == file.path.replace('\\', "/"))
            })
            .collect();
        let actual = skips
            .first()
            .and_then(|s| s["class"].as_str())
            .unwrap_or("scanned");
        let pass = actual == file.expected_coverage
            && file
                .expected_reason
                .as_ref()
                .is_none_or(|reason| skips.iter().any(|s| s["reason"] == *reason));
        if file.kind == "edge" {
            coverage.push(Coverage {
                path: file.path.clone(),
                expected: file.expected_coverage.clone(),
                actual: actual.into(),
                expected_reason: file.expected_reason.clone(),
                actual_reason: skips
                    .first()
                    .and_then(|s| s["reason"].as_str())
                    .map(str::to_owned),
                pass,
            });
        }
    }
    for counts in metrics.values_mut() {
        counts.finish();
    }
    Ok(Evaluation {
        seed: manifest.seed,
        fixture_sha256: String::new(),
        metrics,
        misses,
        spill_bytes,
        spill_predictions,
        negative_false_findings: negative_fp,
        coverage,
        localization_mean: localization.iter().sum::<f64>() / localization.len().max(1) as f64,
        localization_min: localization.into_iter().reduce(f64::min).unwrap_or(0.0),
        residual_findings: 0,
        unchanged_inputs: false,
        redaction_exit: None,
        redaction_missing_outputs: Vec::new(),
        redaction_missing_supported: 0,
        supported_fixtures: manifest.files.iter().filter(|f| f.kind != "edge").count(),
        text_seconds: 0.0,
        image_mean_seconds: 0.0,
        image_max_seconds: 0.0,
        image_timing_failures: 0,
        cpu: String::new(),
        gates: Vec::new(),
        regression_metrics: BTreeMap::new(),
    })
}
fn finish(e: &mut Evaluation) {
    let mut add = |name: String, value: f64, target: String, pass: bool| {
        e.gates.push(Gate {
            name,
            value,
            target,
            pass,
        })
    };
    for (key, m) in &e.metrics {
        if key.starts_with("category/") && key.contains("/severity/") {
            let target = if key.ends_with("/block") { 0.90 } else { 0.80 };
            add(
                format!("Precision {key}"),
                m.precision,
                format!(">= {target:.2}"),
                m.precision >= target,
            );
        }
    }
    let text = e.metrics.get("text").map_or(0.0, |m| m.recall);
    add("Text recall".into(), text, ">= 0.95".into(), text >= 0.95);
    for (key, metric) in &e.metrics {
        if key.starts_with("text/category/") {
            add(
                format!("Recall {key}"),
                metric.recall,
                ">= 0.95".into(),
                metric.recall >= 0.95,
            );
        }
    }
    for (scale, target) in [(1, 0.85), (2, 0.95)] {
        let value = e
            .metrics
            .get(&format!("image/scale/{scale}"))
            .map_or(0.0, |m| m.recall);
        add(
            format!("OCR recall @{scale}x"),
            value,
            format!(">= {target:.2}"),
            value >= target,
        );
        for category in ["email", "payment_card", "phone"] {
            let recall = e
                .metrics
                .get(&format!("image/scale/{scale}/category/{category}"))
                .map_or(0.0, |metric| metric.recall);
            add(
                format!("OCR {category} recall @{scale}x"),
                recall,
                format!(">= {target:.2}"),
                recall >= target,
            );
        }
    }
    add(
        "S5 residual findings".into(),
        e.residual_findings as f64,
        "= 0; inputs unchanged".into(),
        e.residual_findings == 0 && e.unchanged_inputs,
    );
    add(
        "50 MB text seconds".into(),
        e.text_seconds,
        "< 2.0".into(),
        e.text_seconds < 2.0,
    );
    add(
        "S5 supported copies retained".into(),
        e.redaction_missing_supported as f64,
        "= 0".into(),
        e.redaction_missing_supported == 0,
    );
    add(
        "Warm image max seconds".into(),
        e.image_max_seconds,
        "<= 2.5; no errors".into(),
        e.image_max_seconds <= 2.5 && e.image_timing_failures == 0,
    );
    add(
        "Coverage class correctness".into(),
        ratio(
            e.coverage.iter().filter(|c| c.pass).count(),
            e.coverage.len(),
        ),
        "= 1.0".into(),
        e.coverage.iter().all(|c| c.pass),
    );
    for (key, m) in &e.metrics {
        if key.starts_with("off-spec/") {
            continue;
        }
        e.regression_metrics
            .insert(format!("{key}/precision"), m.precision);
        e.regression_metrics
            .insert(format!("{key}/recall"), m.recall);
    }
    e.regression_metrics
        .insert("localization/mean".into(), e.localization_mean);
    e.regression_metrics
        .insert("localization/min".into(), e.localization_min);
    e.regression_metrics.insert(
        "coverage/correct".into(),
        ratio(
            e.coverage.iter().filter(|c| c.pass).count(),
            e.coverage.len(),
        ),
    );
    e.regression_metrics.insert(
        "redaction/clean".into(),
        if e.residual_findings == 0 && e.unchanged_inputs {
            1.0
        } else {
            0.0
        },
    );
    e.regression_metrics.insert(
        "redaction/supported-retained".into(),
        if e.redaction_missing_supported == 0 {
            1.0
        } else {
            0.0
        },
    );
    e.regression_metrics.insert(
        "redaction/retention-fraction".into(),
        ratio(
            e.supported_fixtures
                .saturating_sub(e.redaction_missing_supported),
            e.supported_fixtures,
        ),
    );
    let support = e.metrics.get("overall").map_or(0, |m| m.tp + m.fn_);
    e.regression_metrics.insert(
        "redaction/residual-free-fraction".into(),
        1.0 - e.residual_findings as f64 / support.max(1) as f64,
    );
    e.regression_metrics.insert(
        "redaction/inputs-unchanged".into(),
        if e.unchanged_inputs { 1.0 } else { 0.0 },
    );
}
fn regressions(baseline: &BTreeMap<String, f64>, current: &BTreeMap<String, f64>) -> Vec<String> {
    baseline
        .iter()
        .filter(|(k, v)| current.get(*k).is_none_or(|n| n + 0.0100000001 < **v))
        .map(|(k, _)| k.clone())
        .collect()
}
fn cpu_description() -> String {
    let name = std::env::var("PROCESSOR_IDENTIFIER")
        .ok()
        .or_else(|| {
            fs::read_to_string("/proc/cpuinfo").ok().and_then(|s| {
                s.lines()
                    .find(|l| l.starts_with("model name"))
                    .and_then(|l| l.split_once(':'))
                    .map(|(_, v)| v.trim().to_owned())
            })
        })
        .unwrap_or_else(|| std::env::consts::ARCH.into());
    format!(
        "{name}; {}; {} logical CPUs; ORT CPU, 4 intra-op/1 inter-op threads",
        std::env::consts::OS,
        std::thread::available_parallelism().map_or(1, usize::from)
    )
}
fn render(e: &Evaluation) -> String {
    let mut s = format!(
        "# Independent corpus evaluation\n\nGenerated by `cargo xtask eval`; seed {}. Corpus SHA-256 `{}`. Synthetic fixtures only.\n\n| Gate | Measured | Target | Status |\n|---|---:|---|---|\n",
        e.seed, e.fixture_sha256
    );
    for g in &e.gates {
        s += &format!(
            "| {} | {:.4} | {} | {} |\n",
            g.name,
            g.value,
            g.target,
            if g.pass { "PASS" } else { "FAIL" }
        );
    }
    s += "\n| Group | TP | FP | FN | Precision | Recall |\n|---|---:|---:|---:|---:|---:|\n";
    for (key, m) in &e.metrics {
        s += &format!(
            "| {key} | {} | {} | {} | {:.4} | {:.4} |\n",
            m.tp, m.fp, m.fn_, m.precision, m.recall
        );
    }
    s += &format!(
        "\nSpill outside text extents: {} bytes across {} predictions (diagnostic only). Off-spec groups are excluded from gates and regression metrics.\n",
        e.spill_bytes, e.spill_predictions
    );
    let mut causes = BTreeMap::<&str, usize>::new();
    for miss in &e.misses {
        *causes.entry(&miss.cause).or_default() += 1;
    }
    if !causes.is_empty() {
        s += "\nFalse-negative causes from public JSON geometry (no detector internals inspected):\n\n";
        for (cause, count) in causes {
            s += &format!("- {cause}: {count}\n");
        }
        s += "\nPer-file masked diagnostics are in `target/eval.json` (dev) or `target/eval-test.json` (test). These classify observed mismatches, not detector-internal root causes.\n";
    }
    s += &format!(
        "\nFalse findings on negative regions: {}. OCR localization on matched plants: mean {:.4}, minimum {:.4}.\n\n",
        e.negative_false_findings, e.localization_mean, e.localization_min
    );
    for scale in [1, 2] {
        if let Some(m) = e
            .metrics
            .get(&format!("image/scale/{scale}/category/phone"))
        {
            s += &format!(
                "Phone OCR @{scale}x: {}/{} ({:.2}%).\n\n",
                m.tp,
                m.tp + m.fn_,
                m.recall * 100.0
            );
        }
    }
    s += "| Edge fixture | Expected class/reason | Actual class/reason | Status |\n|---|---|---|---|\n";
    for c in &e.coverage {
        s += &format!(
            "| {} | {} / {} | {} / {} | {} |\n",
            c.path,
            c.expected,
            c.expected_reason.as_deref().unwrap_or("none"),
            c.actual,
            c.actual_reason.as_deref().unwrap_or("none"),
            if c.pass { "PASS" } else { "FAIL" }
        );
    }
    s += &format!(
        "\nRedaction exit: {:?}; residual findings: {}; inputs unchanged: {}. Omitted/deleted outputs: {} (unsupported or failed copies are not counted as successful redactions).\n\n",
        e.redaction_exit,
        e.residual_findings,
        e.unchanged_inputs,
        e.redaction_missing_outputs.len()
    );
    for path in &e.redaction_missing_outputs {
        s += &format!("- `{path}`\n");
    }
    s += &format!(
        "\nPerformance: generated 50,000,000-byte log {:.4} s ({:.2} MB/s), warm images mean/max {:.4}/{:.4} s, {} failures. CPU: {}.\n\n",
        e.text_seconds,
        50.0 / e.text_seconds,
        e.image_mean_seconds,
        e.image_max_seconds,
        e.image_timing_failures,
        e.cpu
    );
    s += "The release CLI scans an isolated byte-for-byte staging copy of manifest fixtures with `scan dir --json - --include-reserved --no-allow`. Generator, labels, patterns and results are metadata, not scan targets. The non-Git working directory permits loading the canonical corpus pattern file without weakening the product repository-pattern restriction. Text matches require exact category/severity and >=90% of core bytes covered; spill outside extent is diagnostic only; image matches require >=98% union glyph area under the product's already padded box. Matching is one-to-one. Empty precision/recall denominators equal 1.0.\n\nWarm image timing uses the unchanged product ImagePipeline after one excluded warmup, including decode/OCR/detection and excluding process/model startup. Text timing includes release process startup and uses only a max_text_mib=64 limit override; no detector tuning. Measurements describe this host, not laptop or CI performance. Absolute failures are findings, never repaired by changing fixtures. `--ci` compares quality metrics against the committed baseline with a one percentage point tolerance; speed is reported separately because host timing varies.\n";
    s += "\nRedaction regression metrics include the supported-copy retention fraction, input immutability, and 1 minus residual finding count divided by total planted support (not clamped, so worsening remains measurable even above support). Binary gate-state metrics also preserve a regression from clean to failing.\n";
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn matching_reassigns_to_avoid_greedy_false_negatives() {
        let candidates = vec![
            vec![(0, 1.), (1, 1.)],
            vec![(0, 1.), (2, 1.)],
            vec![(0, 1.), (2, 1.)],
        ];
        let mut owners = vec![None; 3];
        for label in 0..3 {
            assert!(assign_match(
                label,
                &candidates,
                &mut owners,
                &mut [false; 3]
            ));
        }
        assert!(owners.iter().all(Option::is_some));
    }
    #[test]
    fn scorer_counts_duplicate_prediction_and_negative_region() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("fixture.txt"), b"abcdefghijklmnopqrst").unwrap();
        let manifest: Manifest = serde_json::from_value(serde_json::json!({
            "version":2,"seed":20261027,"files":[{
                "path":"fixture.txt","kind":"text","context":"test","expected_coverage":"scanned",
                "labels":[{"category":"email","severity":"warn","core":{"start":0,"end":10},"extent":{"start":0,"end":10}}]
            }]
        }))
        .unwrap();
        let finding = |start, end| serde_json::json!({"source":{"path":"fixture.txt"},"category":"email","severity":"warn","location":{"kind":"text","line":1,"col_start":start,"col_end":end}});
        let report = serde_json::json!({"findings":[finding(1,11),finding(1,11),finding(16,21)],"coverage":{"skipped":[]}});
        let result = score(&manifest, dir.path(), &report).unwrap();
        let overall = &result.metrics["overall"];
        assert_eq!((overall.tp, overall.fp, overall.fn_), (1, 2, 0));
        assert_eq!(result.negative_false_findings, 1);
    }
    #[test]
    fn core_coverage_spill_and_off_spec_are_independent() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("fixture.txt"), b"abcdefghijklmnopqrst").unwrap();
        let manifest: Manifest = serde_json::from_value(serde_json::json!({
            "version":2,"seed":20261027,"files":[{
                "path":"fixture.txt","kind":"text","context":"test","expected_coverage":"scanned",
                "labels":[
                    {"category":"email","severity":"warn","core":{"start":3,"end":7},"extent":{"start":2,"end":8}},
                    {"category":"phone","severity":"warn","core":{"start":12,"end":16},"extent":{"start":10,"end":18},"off_spec":"phone/national-no-country-code"}
                ]
            }]
        })).unwrap();
        let report = serde_json::json!({"findings":[
            {"source":{"path":"fixture.txt"},"category":"email","severity":"warn","location":{"line":1,"col_start":1,"col_end":11}},
            {"source":{"path":"fixture.txt"},"category":"phone","severity":"warn","location":{"line":1,"col_start":13,"col_end":15}}
        ],"coverage":{"skipped":[]}});
        let mut result = score(&manifest, dir.path(), &report).unwrap();
        assert_eq!(
            (
                result.metrics["overall"].tp,
                result.metrics["overall"].fp,
                result.metrics["overall"].fn_
            ),
            (1, 0, 0)
        );
        assert_eq!(result.spill_bytes, 4);
        assert_eq!(result.spill_predictions, 1);
        let off = &result.metrics["off-spec/phone/national-no-country-code"];
        assert_eq!((off.tp, off.fp, off.fn_), (0, 1, 1));
        finish(&mut result);
        assert!(
            !result
                .regression_metrics
                .keys()
                .any(|key| key.starts_with("off-spec/"))
        );
    }
    #[test]
    fn spans_require_ninety_percent_of_core() {
        assert_eq!(span_overlap((10, 20), (10, 19)), 0.9);
        assert_eq!(span_overlap((10, 20), (0, 30)), 1.0);
    }
    #[test]
    fn unicode_columns_become_byte_spans() {
        let l = serde_json::json!({"line":2,"col_start":3,"col_end":5});
        assert_eq!(text_span("a\nΩ éz".as_bytes(), &l), Some((5, 8)));
    }
    #[test]
    fn glyph_union_does_not_double_count_overlap() {
        assert_eq!(area_union(&[[0., 0., 10., 10.], [5., 0., 10., 10.]]), 150.);
        assert!(
            (box_coverage(
                &[[0., 0., 10., 10.], [5., 0., 10., 10.]],
                [0., 0., 7.5, 10.]
            ) - 0.5)
                .abs()
                < 1e-9
        );
    }
    #[test]
    fn regression_tolerance_and_missing_metrics() {
        let b = BTreeMap::from([("a".into(), 0.9)]);
        assert!(regressions(&b, &BTreeMap::from([("a".into(), 0.89)])).is_empty());
        assert_eq!(
            regressions(&b, &BTreeMap::from([("a".into(), 0.889)])).len(),
            1
        );
        assert_eq!(regressions(&b, &BTreeMap::new()).len(), 1);
    }
}
