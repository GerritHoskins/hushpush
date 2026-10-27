use sha2::{Digest, Sha256};
use std::{
    error::Error,
    fs,
    path::{Component, Path, PathBuf},
    process::Command,
};

type Result<T> = std::result::Result<T, Box<dyn Error>>;

const DET: &str = "https://huggingface.co/PaddlePaddle/PP-OCRv5_mobile_det_onnx/resolve/e6f4fa85f00e168c862bc462aebca69eef9b3d3d";
const REC_URL: &str = "https://huggingface.co/PaddlePaddle/PP-OCRv5_mobile_rec_onnx/resolve/ed152b8b495f84de93cda5709d768548a9127622";

pub fn run() -> Result<()> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or("workspace root missing")?;
    let models = root.join("models");
    let cache = models.join(".cache");
    fs::create_dir_all(&cache)?;
    for (base, source, target, hash) in [
        (
            DET,
            "inference.onnx",
            "mobile_det_inference.onnx",
            "a431985659dc921974177a95adcfbb90fd9e51989a5e04d70d0b75f597b6e61d",
        ),
        (
            REC_URL,
            "inference.onnx",
            "mobile_rec_inference.onnx",
            "da72dc72ca4dc220df0dfde68c1dedc31c58d3e76a25871122e5056227d50092",
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
    let dict_hash = "1ea29636956177e400af712d9782e7693f3fb25f98617bed10479d2965a836fd";
    check_bytes(dictionary.as_bytes(), dict_hash)?;
    fs::write(models.join("mobile_dict.txt"), dictionary)?;
    println!(
        "mobile_dict.txt: {} bytes (SHA-256 verified)",
        fs::metadata(models.join("mobile_dict.txt"))?.len()
    );
    install_runtime(root, &models, &cache)?;
    println!("Models: {}", models.display());
    println!(
        "PP-OCRv5 models/dictionary: Apache-2.0, see mobile_*_README.md; ONNX Runtime: MIT, see ort/ notices."
    );
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

fn tar() -> Command {
    if cfg!(windows) {
        Command::new(
            PathBuf::from(std::env::var_os("WINDIR").unwrap_or_else(|| "C:\\Windows".into()))
                .join("System32/tar.exe"),
        )
    } else {
        Command::new("tar")
    }
}

fn install_runtime(root: &Path, models: &Path, cache: &Path) -> Result<()> {
    let (platform, suffix, hash, library) = match (std::env::consts::OS, std::env::consts::ARCH) {
        ("windows", "x86_64") => (
            "win-x64",
            "zip",
            "174c616efc0271194488642a72f1a514e01487da4dfe84c49296d66e40ebe0da",
            "onnxruntime.dll",
        ),
        ("linux", "x86_64") => (
            "linux-x64",
            "tgz",
            "8344d55f93d5bc5021ce342db50f62079daf39aaafb5d311a451846228be49b3",
            "libonnxruntime.so",
        ),
        ("linux", "aarch64") => (
            "linux-aarch64",
            "tgz",
            "bb76395092d150b52c7092dc6b8f2fe4d80f0f3bf0416d2f269193e347e24702",
            "libonnxruntime.so",
        ),
        ("macos", "x86_64") => (
            "osx-x86_64",
            "tgz",
            "e4ec94a7696de74fb1b12846569aa94e499958af6ffa186022cfde16c9d617f0",
            "libonnxruntime.dylib",
        ),
        ("macos", "aarch64") => (
            "osx-arm64",
            "tgz",
            "cab6dcbd77e7ec775390e7b73a8939d45fec3379b017c7cb74f5b204c1a1cc07",
            "libonnxruntime.dylib",
        ),
        _ => return Err("unsupported host OS/architecture for the pinned CPU runtime".into()),
    };
    let package = format!("onnxruntime-{platform}-1.22.0");
    let archive = cache.join(format!("{package}.{suffix}"));
    download(
        &format!(
            "https://github.com/microsoft/onnxruntime/releases/download/v1.22.0/{package}.{suffix}"
        ),
        &archive,
        hash,
    )?;
    let listing = tar().arg("-tf").arg(&archive).output()?;
    if !listing.status.success() {
        return Err("cannot list runtime archive (system tar is required)".into());
    }
    for name in std::str::from_utf8(&listing.stdout)?.lines() {
        let path = Path::new(name);
        if !name.starts_with(&format!("{package}/"))
            || path.components().any(|c| {
                matches!(
                    c,
                    Component::ParentDir | Component::RootDir | Component::Prefix(_)
                )
            })
        {
            return Err("unsafe runtime archive path".into());
        }
    }
    let destination = models.join("ort");
    fs::create_dir_all(&destination)?;
    if !tar()
        .arg("-xf")
        .arg(&archive)
        .arg("-C")
        .arg(&destination)
        .status()?
        .success()
    {
        return Err("runtime extraction failed".into());
    }
    let runtime_dir = destination.join(&package).join("lib");
    let target = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .map(|p| if p.is_absolute() { p } else { root.join(p) })
        .unwrap_or_else(|| root.join("target"));
    // Integration tests execute under deps/, so they need the same CPU runtime beside them.
    for profile in ["debug", "release"] {
        for directory in [target.join(profile), target.join(profile).join("deps")] {
            fs::create_dir_all(&directory)?;
            for file in fs::read_dir(&runtime_dir)? {
                let file = file?;
                let name = file.file_name();
                let name_str = name.to_string_lossy();
                if name_str.ends_with(".dll")
                    || name_str.contains(".so")
                    || name_str.ends_with(".dylib")
                {
                    let destination = directory.join(name);
                    let source = fs::read(file.path())?;
                    if fs::read(&destination).ok().as_deref() != Some(source.as_slice()) {
                        fs::write(destination, source)?;
                    }
                }
            }
        }
    }
    println!("CPU ONNX Runtime 1.22.0 installed beside debug/release executables.");
    println!(
        "Alternative ORT_DYLIB_PATH={}",
        runtime_dir.join(library).display()
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
