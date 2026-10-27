use std::{path::Path, process::Command};

const IMAGE: &str =
    "rustlang/rust@sha256:7fcf69d30fc42bd6f652305d904cfefa8818377401ca912717fe935953ff9711";

pub fn run(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let mut minutes = None;
    let mut target = None;
    let mut docker = false;
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--docker" => docker = true,
            "--minutes" => minutes = Some(args.next().ok_or("missing minutes")?.parse::<u32>()?),
            "--target" => target = Some(args.next().ok_or("missing target")?.as_str()),
            _ => return Err("usage: cargo xtask fuzz --docker --minutes N [--target T]".into()),
        }
    }
    let minutes = minutes
        .filter(|n| *n > 0)
        .ok_or("minutes must be positive")?;
    if !docker {
        return Err("fuzzing requires --docker".into());
    }
    if target.is_some_and(|t| !super_target(t)) {
        return Err("unknown fuzz target".into());
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or("workspace unavailable")?
        .canonicalize()?;
    let mut command = Command::new("docker");
    command.args(["run", "--rm", "--mount"]);
    let root = root.to_string_lossy();
    let root = root.strip_prefix(r"\\?\").unwrap_or(&root);
    command.arg(format!("type=bind,source={root},target=/repo"));
    command.args([
        "--mount",
        "type=volume,source=hushpush-fuzz-target,target=/fuzz-target",
        "-w",
        "/repo",
        IMAGE,
        "bash",
        "/repo/scripts/fuzz-docker.sh",
        "--inside",
        "--minutes",
    ]);
    command.arg(minutes.to_string());
    if let Some(target) = target {
        command.args(["--target", target]);
    }
    if !command.status()?.success() {
        return Err("Docker fuzz run failed; inspect fuzz/artifacts and fuzz/logs".into());
    }
    Ok(())
}

fn super_target(target: &str) -> bool {
    matches!(
        target,
        "classify_decode_detect" | "image_decode" | "config_parse" | "git_parsers" | "redact_text"
    )
}
