use sha2::{Digest, Sha256};
use std::{env, fs, path::PathBuf};
mod model_pins;
fn main() {
    println!("cargo:rerun-if-env-changed=HUSHPUSH_MODELS_DIR");
    println!("cargo:rerun-if-changed=model_pins.rs");
    if env::var_os("CARGO_FEATURE_BUNDLED_MODELS").is_none() {
        return;
    }
    let directory = env::var_os("HUSHPUSH_MODELS_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../models"));
    let out = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR"));
    for (name, hash) in [
        ("mobile_det_inference.onnx", model_pins::DET_HASH),
        ("mobile_rec_inference.onnx", model_pins::REC_HASH),
        ("mobile_dict.txt", model_pins::DICT_HASH),
    ] {
        let path = directory.join(name);
        println!("cargo:rerun-if-changed={}", path.display());
        let bytes = fs::read(&path).unwrap_or_else(|_| panic!("bundled-models: missing {name}; run cargo xtask fetch-models or set HUSHPUSH_MODELS_DIR"));
        assert_eq!(
            format!("{:x}", Sha256::digest(&bytes)),
            hash,
            "bundled-models: SHA-256 mismatch for {name}; fetch pinned models again"
        );
        fs::write(out.join(name), bytes).expect("copy verified model to OUT_DIR");
    }
}
