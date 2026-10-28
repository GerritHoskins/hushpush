#[path = "../../crates/hushpush-core/model_pins.rs"]
mod model_pins;
use sha2::{Digest, Sha256};
use std::{error::Error, fs, path::Path, process::Command};

type Result<T> = std::result::Result<T, Box<dyn Error>>;

const DET: &str = "https://huggingface.co/PaddlePaddle/PP-OCRv5_mobile_det_onnx/resolve/e6f4fa85f00e168c862bc462aebca69eef9b3d3d";
const REC_URL: &str = "https://huggingface.co/PaddlePaddle/PP-OCRv5_mobile_rec_onnx/resolve/ed152b8b495f84de93cda5709d768548a9127622";

pub fn run() -> Result<()> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or("workspace root missing")?;
    let models = root.join("models");
    fs::create_dir_all(&models)?;
    for (base, source, target, hash) in [
        (
            DET,
            "inference.onnx",
            "mobile_det_inference.onnx",
            model_pins::DET_HASH,
        ),
        (
            REC_URL,
            "inference.onnx",
            "mobile_rec_inference.onnx",
            model_pins::REC_HASH,
        ),
        (
            REC_URL,
            "inference.yml",
            "mobile_rec_inference.yml",
            "5dfeb2777f6d0db8177d8128a8acfcf6e6276dc4ac73ea3bf0dc06d6a5e85d8e",
        ),
        (
            DET,
            "README.md",
            "mobile_det_README.md",
            "951c155673ac6c15feb85857c9024a62266e5956d15319ce578a8f508f399d66",
        ),
        (
            REC_URL,
            "README.md",
            "mobile_rec_README.md",
            "7331c129fd5024b6a0abd1f510cfdfb2d7714fdcdbb00df21a0cb8b48e6683ee",
        ),
    ] {
        download(&format!("{base}/{source}"), &models.join(target), hash)?;
    }
    let yaml: DictionaryConfig = serde_saphyr::from_str(&fs::read_to_string(
        models.join("mobile_rec_inference.yml"),
    )?)?;
    let entries = yaml.post_process.character_dict;
    let mut dictionary = String::new();
    for entry in entries {
        dictionary.push_str(&entry);
        // The pinned spike dictionary uses CRLF on every host.
        dictionary.push_str("\r\n");
    }
    let dict_hash = model_pins::DICT_HASH;
    check_bytes(dictionary.as_bytes(), dict_hash)?;
    fs::write(models.join("mobile_dict.txt"), dictionary)?;
    println!(
        "mobile_dict.txt: {} bytes (SHA-256 verified)",
        fs::metadata(models.join("mobile_dict.txt"))?.len()
    );
    println!("Models: {}", models.display());
    println!("PP-OCRv5 models/dictionary: Apache-2.0, see mobile_*_README.md.");
    Ok(())
}

fn check_bytes(bytes: &[u8], expected: &str) -> Result<()> {
    if format!("{:x}", Sha256::digest(bytes)) != expected {
        return Err("SHA-256 verification failed".into());
    }
    Ok(())
}

fn download(url: &str, path: &Path, hash: &str) -> Result<()> {
    if path.is_file() && check_bytes(&fs::read(path)?, hash).is_ok() {
        println!(
            "{}: {} bytes (SHA-256 verified, cached)",
            path.file_name().unwrap_or_default().to_string_lossy(),
            fs::metadata(path)?.len()
        );
        return Ok(());
    }
    let temporary = path.with_extension("download");
    let mut curl = Command::new(if cfg!(windows) { "curl.exe" } else { "curl" });
    curl.args([
        "--fail",
        "--location",
        "--retry",
        "3",
        "--connect-timeout",
        "30",
        "--max-time",
        "600",
        "--silent",
        "--show-error",
        "--proto",
        "=https",
        "--proto-redir",
        "=https",
    ]);
    if cfg!(windows) {
        // Keep certificate verification while tolerating an unreachable Windows CRL server.
        curl.arg("--ssl-revoke-best-effort");
    }
    if !curl
        .arg("--output")
        .arg(&temporary)
        .arg(url)
        .status()?
        .success()
    {
        return Err(format!("download failed: {url}").into());
    }
    check_bytes(&fs::read(&temporary)?, hash)?;
    if path.exists() {
        fs::remove_file(path)?;
    }
    fs::rename(&temporary, path)?;
    println!(
        "{}: {} bytes (SHA-256 verified)",
        path.file_name().unwrap_or_default().to_string_lossy(),
        fs::metadata(path)?.len()
    );
    Ok(())
}

#[derive(serde::Deserialize)]
struct DictionaryConfig {
    #[serde(rename = "PostProcess")]
    post_process: DictionaryPostProcess,
}

#[derive(serde::Deserialize)]
struct DictionaryPostProcess {
    character_dict: Vec<String>,
}
