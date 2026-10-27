use anyhow::{Context, Result, ensure};
use ocrs::{ImageSource, OcrEngine, OcrEngineParams};
use rten::Model;
use serde_json::json;
use std::{fs, path::PathBuf, time::Instant};

// Even synthetic recognized text must never reach diagnostic output.
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

fn recognize(engine: &OcrEngine, bytes: &[u8], width: u32, height: u32) -> Result<usize> {
    let input = engine.prepare_input(ImageSource::from_bytes(bytes, (width, height))?)?;
    let words = engine.detect_words(&input)?;
    let lines = engine.find_text_lines(&input, &words);
    let text = Sensitive(engine.recognize_text(&input, &lines)?);
    Ok(text.0.len())
}

fn main() -> Result<()> {
    std::panic::set_hook(Box::new(|_| eprintln!("benchmark panic [redacted]")));
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    ensure!(args.len() >= 4, "usage: bench DETECT REC OUTPUT IMAGE...");
    let engine = OcrEngine::new(OcrEngineParams {
        detection_model: Some(Model::load(fs::read(&args[0])?)?),
        recognition_model: Some(Model::load(fs::read(&args[1])?)?),
        ..Default::default()
    })?;
    let mut results = Vec::new();
    for (i, path) in args[3..].iter().enumerate() {
        let image = image::open(path)
            .context("synthetic image decode failed")?
            .into_rgb8();
        if i == 0 {
            recognize(&engine, image.as_raw(), image.width(), image.height())?;
        }
        let start = Instant::now();
        let lines = recognize(&engine, image.as_raw(), image.width(), image.height())?;
        let seconds = start.elapsed().as_secs_f64();
        let id = PathBuf::from(path)
            .file_stem()
            .context("missing image ID")?
            .to_string_lossy()
            .into_owned();
        results.push(json!({"id": id, "seconds": seconds, "lines": lines, "width": image.width(), "height": image.height()}));
    }
    fs::write(&args[2], serde_json::to_vec_pretty(&results)?)?;
    eprintln!("Measured {} synthetic images", results.len());
    Ok(())
}
