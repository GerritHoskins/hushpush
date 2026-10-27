use std::{
    fs,
    io::{self, Write},
    path::Path,
    process::{Command, Stdio},
};

use hushpush_core::ExitCode;

fn check_target(path: &Path) -> Result<(), ExitCode> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_symlink() || metadata.is_dir() => {
            Err(ExitCode::UsageOrConfigError)
        }
        Ok(_) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(_) => Err(ExitCode::InternalError),
    }
}

pub fn write(
    path: &Path,
    serialize: impl FnOnce(&mut dyn Write) -> io::Result<()>,
) -> Result<(), ExitCode> {
    check_target(path)?;
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    // tempfile creates Unix files with 0600 at open time; persist replaces by atomic rename.
    let mut temp = tempfile::Builder::new()
        .prefix(".hushpush-report-")
        .tempfile_in(parent)
        .map_err(|_| ExitCode::InternalError)?;
    serialize(temp.as_file_mut()).map_err(|_| ExitCode::InternalError)?;
    temp.as_file_mut()
        .sync_all()
        .map_err(|_| ExitCode::InternalError)?;
    check_target(path)?;
    temp.persist(path).map_err(|_| ExitCode::InternalError)?;
    if unignored(path) {
        eprintln!("report file is not git-ignored; it lists paths and fingerprints");
    }
    Ok(())
}

fn unignored(path: &Path) -> bool {
    let Ok(path) = fs::canonicalize(path) else {
        return false;
    };
    let Some(parent) = path.parent() else {
        return false;
    };
    if hushpush_core::source::git::working_tree_root(parent, false).is_none() {
        return false;
    }
    let mut command = Command::new("git");
    for (key, _) in std::env::vars_os() {
        if key
            .to_string_lossy()
            .to_ascii_uppercase()
            .starts_with("GIT_")
        {
            command.env_remove(key);
        }
    }
    command
        .current_dir(parent)
        .args(["-c", "core.fsmonitor=false", "check-ignore", "-q", "--"])
        .arg(path)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env(
            "GIT_CONFIG_GLOBAL",
            if cfg!(windows) { "NUL" } else { "/dev/null" },
        )
        .env("GIT_OPTIONAL_LOCKS", "0")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.code() == Some(1))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failed_serialization_keeps_previous_file_and_removes_temp() {
        let dir = tempfile::Builder::new()
            .prefix("report-write-failure-")
            .tempdir_in(env!("CARGO_MANIFEST_DIR"))
            .unwrap();
        let target = dir.path().join("report");
        fs::write(&target, "previous report").unwrap();
        let result = write(&target, |out| {
            out.write_all(b"partial output")?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let temp = fs::read_dir(dir.path())
                    .unwrap()
                    .map(Result::unwrap)
                    .find(|entry| {
                        entry
                            .file_name()
                            .to_string_lossy()
                            .starts_with(".hushpush-report-")
                    })
                    .unwrap();
                assert_eq!(temp.metadata().unwrap().permissions().mode() & 0o777, 0o600);
            }
            Err(io::Error::other("synthetic write failure"))
        });
        assert_eq!(result, Err(ExitCode::InternalError));
        assert_eq!(fs::read(&target).unwrap(), b"previous report");
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
    }
}
