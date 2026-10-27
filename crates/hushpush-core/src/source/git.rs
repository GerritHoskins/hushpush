//! Immutable Git object sources. Paths are display labels, never filesystem inputs.
use std::{
    io::{BufRead, BufReader, Read, Write},
    path::Path,
    process::{Child, ChildStdin, ChildStdout, Command, Stdio},
};

use super::{Origin, scan_bytes, skip};
use crate::{
    ExitCode,
    config::Config,
    report::{Report, SkipReason},
};

#[derive(Debug, thiserror::Error)]
pub enum GitError {
    #[error("git is unavailable")]
    Unavailable,
    #[error("not a git repository")]
    NotRepository,
    #[error("repository ownership check failed (see git safe.directory)")]
    Ownership,
    #[error("invalid pre-push input")]
    InvalidInput,
    #[error("expected a valid A..B commit range")]
    InvalidRange,
    #[error("scan push requires a current branch (detached or unborn HEAD)")]
    NoBranch,
    #[error("git operation failed")]
    Failed,
}

impl GitError {
    pub fn exit_code(&self) -> ExitCode {
        match self {
            Self::Unavailable
            | Self::NotRepository
            | Self::Ownership
            | Self::InvalidInput
            | Self::InvalidRange
            | Self::NoBranch => ExitCode::UsageOrConfigError,
            Self::Failed => ExitCode::InternalError,
        }
    }
}

const ROUTING_VARIABLES: &[&str] = &[
    "GIT_DIR",
    "GIT_WORK_TREE",
    "GIT_INDEX_FILE",
    "GIT_COMMON_DIR",
    "GIT_OBJECT_DIRECTORY",
    "GIT_ALTERNATE_OBJECT_DIRECTORIES",
    "GIT_QUARANTINE_PATH",
    "GIT_NAMESPACE",
    "GIT_CEILING_DIRECTORIES",
    "GIT_DISCOVERY_ACROSS_FILESYSTEM",
];

pub fn working_tree_root(root: &Path, routing: bool) -> Option<std::path::PathBuf> {
    let output = command(root, routing)
        .ok()?
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8(output.stdout).ok()?;
    Some(std::path::PathBuf::from(
        text.trim_end_matches(['\r', '\n']),
    ))
}

pub(crate) fn command(root: &Path, inherit_routing: bool) -> Result<Command, GitError> {
    let mut command = Command::new("git");
    for (key, value) in std::env::vars_os() {
        let name = key.to_string_lossy().to_ascii_uppercase();
        if name.starts_with("GIT_") {
            command.env_remove(&key);
            if inherit_routing && ROUTING_VARIABLES.contains(&name.as_str()) {
                if matches!(name.as_str(), "GIT_INDEX_FILE" | "GIT_DIR") {
                    // Git may run in a different directory from the hook's caller.
                    let path = std::path::absolute(value).map_err(|_| GitError::Failed)?;
                    command.env(&key, path);
                } else {
                    command.env(&key, value);
                }
            }
        }
    }
    command
        .current_dir(root)
        .args([
            "--no-pager",
            "--no-replace-objects",
            "-c",
            "core.abbrev=no",
            "-c",
            "core.fsmonitor=false",
        ])
        .env("GIT_OPTIONAL_LOCKS", "0")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env(
            "GIT_CONFIG_GLOBAL",
            if cfg!(windows) { "NUL" } else { "/dev/null" },
        )
        .env("GIT_NO_LAZY_FETCH", "1")
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("LC_ALL", "C")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    Ok(command)
}

struct Entry {
    path: String,
    oid: String,
    symlink: bool,
    submodule: bool,
}

fn entries(raw: &[u8]) -> Result<Vec<Entry>, GitError> {
    let mut fields = raw.split(|b| *b == 0);
    let mut entries = Vec::new();
    while let Some(header) = fields.next() {
        if header.is_empty() {
            break;
        }
        let path = fields.next().ok_or(GitError::Failed)?;
        let header = std::str::from_utf8(header).map_err(|_| GitError::Failed)?;
        let parts: Vec<_> = header.split(' ').collect();
        if parts.len() != 5 || !parts[0].starts_with(':') {
            return Err(GitError::Failed);
        }
        if !matches!(parts[4], "A" | "M" | "T") {
            continue;
        }
        let oid = parts[3];
        if !matches!(oid.len(), 40 | 64) || !oid.bytes().all(|c| c.is_ascii_hexdigit()) {
            return Err(GitError::Failed);
        }
        entries.push(Entry {
            path: String::from_utf8_lossy(path).into_owned(),
            oid: oid.into(),
            symlink: parts[1] == "120000",
            submodule: parts[1] == "160000",
        });
    }
    Ok(entries)
}

struct Batch {
    child: Child,
    input: Option<ChildStdin>,
    output: BufReader<ChildStdout>,
}

impl Batch {
    fn new(root: &Path, inherit_routing: bool, flag: &str) -> Result<Self, GitError> {
        let mut child = command(root, inherit_routing)?
            .args(["cat-file", flag])
            .stdin(Stdio::piped())
            .spawn()
            .map_err(|_| GitError::Failed)?;
        let input = child.stdin.take();
        let output = BufReader::new(child.stdout.take().ok_or(GitError::Failed)?);
        Ok(Self {
            child,
            input,
            output,
        })
    }

    fn query(&mut self, oid: &str) -> Result<Option<(String, u64)>, GitError> {
        writeln!(self.input.as_mut().ok_or(GitError::Failed)?, "{oid}")
            .map_err(|_| GitError::Failed)?;
        self.input
            .as_mut()
            .ok_or(GitError::Failed)?
            .flush()
            .map_err(|_| GitError::Failed)?;
        let mut line = String::new();
        self.output
            .by_ref()
            .take(256)
            .read_line(&mut line)
            .map_err(|_| GitError::Failed)?;
        let parts: Vec<_> = line.split_whitespace().collect();
        if parts.len() == 2 && parts[0] == oid && parts[1] == "missing" {
            return Ok(None);
        }
        if parts.len() != 3 || parts[0] != oid {
            return Err(GitError::Failed);
        }
        Ok(Some((
            parts[1].into(),
            parts[2].parse().map_err(|_| GitError::Failed)?,
        )))
    }

    fn header(&mut self, oid: &str) -> Result<u64, GitError> {
        match self.query(oid)? {
            Some((kind, size)) if kind == "blob" => Ok(size),
            _ => Err(GitError::Failed),
        }
    }

    fn finish(mut self) -> Result<(), GitError> {
        self.input.take();
        if self.child.wait().map_err(|_| GitError::Failed)?.success() {
            Ok(())
        } else {
            Err(GitError::Failed)
        }
    }
}

impl Drop for Batch {
    fn drop(&mut self) {
        self.input.take();
        // Reap children even if a protocol or IO error interrupts the scan.
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn oversized_reason(
    root: &Path,
    routing: bool,
    oid: &str,
    symlink: bool,
) -> Result<SkipReason, GitError> {
    if symlink {
        return Ok(SkipReason::TooLarge);
    }
    let mut child = command(root, routing)?
        .args(["cat-file", "blob", oid])
        .spawn()
        .map_err(|_| GitError::Failed)?;
    let mut prefix = [0; 8192];
    let result = child
        .stdout
        .take()
        .ok_or(GitError::Failed)?
        .read_exact(&mut prefix);
    let _ = child.kill();
    let _ = child.wait();
    result.map_err(|_| GitError::Failed)?;
    Ok(match crate::classify::classify(&prefix) {
        crate::classify::ContentKind::Image => SkipReason::ImageTooLarge,
        crate::classify::ContentKind::Binary => SkipReason::Binary,
        crate::classify::ContentKind::LfsPointer => SkipReason::LfsPointer,
        crate::classify::ContentKind::Text => SkipReason::TooLarge,
        crate::classify::ContentKind::Unsupported(reason) => reason,
    })
}

fn repository_error(stderr: &[u8]) -> GitError {
    let message = String::from_utf8_lossy(stderr);
    if message.contains("dubious ownership") {
        GitError::Ownership
    } else if message.contains("not a git repository") {
        GitError::NotRepository
    } else {
        GitError::Failed
    }
}

/// Scan using the caller's Git routing, including hook temporary indexes.
pub fn scan_staged(root: &Path) -> Result<Report, GitError> {
    scan_staged_with_routing(root, true)
}

/// Disable inherited routing when an explicit CLI directory selects the repository.
pub fn scan_staged_with_routing(root: &Path, inherit_routing: bool) -> Result<Report, GitError> {
    scan_staged_with_config(root, inherit_routing, &Config::default())
}

pub fn scan_staged_with_config(
    root: &Path,
    inherit_routing: bool,
    config: &Config,
) -> Result<Report, GitError> {
    let probe = command(root, inherit_routing)?
        .args(["rev-parse", "--git-dir"])
        .stderr(Stdio::piped())
        .output()
        .map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                GitError::Unavailable
            } else {
                GitError::Failed
            }
        })?;
    if !probe.status.success() {
        return Err(repository_error(&probe.stderr));
    }
    let diff = command(root, inherit_routing)?
        .args([
            "diff",
            "--cached",
            "--raw",
            "-z",
            "--no-renames",
            "--no-relative",
            "--no-ext-diff",
            "--no-abbrev",
            "--no-color",
            "--ignore-submodules=none",
        ])
        .output()
        .map_err(|_| GitError::Failed)?;
    if !diff.status.success() {
        return Err(GitError::Failed);
    }
    let entries = entries(&diff.stdout)?;
    let mut report = Report::with_config(config);
    let mut sizes = Batch::new(root, inherit_routing, "--batch-check")?;
    let mut pending = Vec::new();
    for entry in entries {
        if entry.submodule {
            skip(&mut report, entry.path, SkipReason::Submodule);
        } else {
            let size = match sizes.query(&entry.oid)? {
                Some((kind, size)) if kind == "blob" => size,
                None => {
                    skip(&mut report, entry.path, SkipReason::MissingObject);
                    continue;
                }
                _ => return Err(GitError::Failed),
            };
            if size
                > report
                    .config
                    .limits
                    .image_bytes()
                    .max(report.config.limits.text_bytes())
            {
                let reason = oversized_reason(root, inherit_routing, &entry.oid, entry.symlink)?;
                skip(&mut report, entry.path, reason);
            } else {
                pending.push((entry, size));
            }
        }
    }
    sizes.finish()?;
    let mut contents = Batch::new(root, inherit_routing, "--batch")?;
    for (entry, size) in pending {
        if contents.header(&entry.oid)? != size {
            return Err(GitError::Failed);
        }
        let mut bytes = vec![0; size as usize];
        contents
            .output
            .read_exact(&mut bytes)
            .map_err(|_| GitError::Failed)?;
        let mut newline = [0];
        contents
            .output
            .read_exact(&mut newline)
            .map_err(|_| GitError::Failed)?;
        if newline != *b"\n" {
            return Err(GitError::Failed);
        }
        scan_bytes(
            &mut report,
            entry.path,
            &bytes,
            Origin {
                blob_oid: Some(&entry.oid),
                symlink: entry.symlink,
            },
            false,
        );
    }
    contents.finish()?;
    Ok(report)
}

/// Outgoing coverage includes warnings that do not make coverage incomplete.
#[derive(Default, Debug)]
pub struct Outgoing {
    pub report: Report,
    pub ref_updates: usize,
    pub commit_range: Option<String>,
    pub full_history: bool,
    pub nothing_to_push: bool,
}

impl Outgoing {
    pub fn write_terminal(&self, mut out: impl Write) -> std::io::Result<()> {
        if self.nothing_to_push {
            writeln!(out, "Nothing to push")?;
        }
        let label = if let Some(range) = &self.commit_range {
            format!("blobs in {}", crate::report::sanitize_path(range))
        } else {
            format!(
                "blobs in {} ref {}",
                self.ref_updates,
                crate::report::plural(self.ref_updates, "update", "updates")
            )
        };
        self.report.write_terminal_mode(&mut out, &label)?;
        if self.report.coverage.approximate {
            writeln!(
                out,
                "Coverage approximate (over-scan): remote commit unavailable locally"
            )?;
        }
        if self.full_history {
            writeln!(
                out,
                "Warning: no remote-tracking refs; scanning full history can be slow"
            )?;
        }
        Ok(())
    }
}

fn probe(root: &Path, routing: bool) -> Result<(), GitError> {
    let output = command(root, routing)?
        .args(["rev-parse", "--git-dir"])
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                GitError::Unavailable
            } else {
                GitError::Failed
            }
        })?;
    if output.status.success() {
        Ok(())
    } else {
        Err(repository_error(&output.stderr))
    }
}

fn valid_oid(oid: &str) -> bool {
    matches!(oid.len(), 40 | 64) && oid.bytes().all(|b| b.is_ascii_hexdigit())
}

fn zero(oid: &str) -> bool {
    oid.bytes().all(|b| b == b'0')
}

fn resolve(root: &Path, routing: bool, revision: &str) -> Result<Option<String>, GitError> {
    let output = command(root, routing)?
        .args([
            "rev-parse",
            "--verify",
            "--end-of-options",
            &format!("{revision}^{{commit}}"),
        ])
        .output()
        .map_err(|_| GitError::Failed)?;
    if !output.status.success() {
        return Ok(None);
    }
    let oid = String::from_utf8(output.stdout).map_err(|_| GitError::Failed)?;
    let oid = oid.trim();
    if !valid_oid(oid) {
        return Err(GitError::Failed);
    }
    Ok(Some(oid.into()))
}

struct Update {
    local: String,
    remote: String,
}

/// Read hook stdin as data, never as Git revision arguments.
pub fn scan_pre_push(
    root: &Path,
    routing: bool,
    remote: &str,
    input: &str,
) -> Result<Outgoing, GitError> {
    scan_pre_push_with_config(root, routing, remote, input, &Config::default())
}

pub fn scan_pre_push_with_config(
    root: &Path,
    routing: bool,
    remote: &str,
    input: &str,
    config: &Config,
) -> Result<Outgoing, GitError> {
    let updates = parse_pre_push(input)?;
    if updates.is_empty() {
        return Ok(Outgoing {
            nothing_to_push: true,
            report: Report::with_config(config),
            ..Outgoing::default()
        });
    }
    probe(root, routing)?;
    scan_updates(root, routing, remote, updates, config)
}

fn parse_pre_push(input: &str) -> Result<Vec<Update>, GitError> {
    let mut updates = Vec::new();
    for line in input.lines() {
        let fields: Vec<_> = line.split_whitespace().collect();
        if fields.len() != 4 || !valid_oid(fields[1]) || !valid_oid(fields[3]) {
            return Err(GitError::InvalidInput);
        }
        updates.push(Update {
            local: fields[1].into(),
            remote: fields[3].into(),
        });
    }
    Ok(updates)
}

#[cfg(feature = "fuzzing")]
pub(crate) fn fuzz_parsers(data: &[u8]) {
    let _ = entries(data);
    let _ = object_records(data, true);
    let _ = object_records(data, false);
    if let Ok(text) = std::str::from_utf8(data) {
        let _ = parse_pre_push(text);
    }
}

fn scan_updates(
    root: &Path,
    routing: bool,
    remote: &str,
    updates: Vec<Update>,
    config: &Config,
) -> Result<Outgoing, GitError> {
    // Prefix filtering avoids interpreting a URL or remote name as a ref glob.
    let refs = command(root, routing)?
        .args(["for-each-ref", "--format=%(refname)"])
        .output()
        .map_err(|_| GitError::Failed)?;
    if !refs.status.success() {
        return Err(GitError::Failed);
    }
    let prefix = format!("refs/remotes/{remote}/");
    let tracking = String::from_utf8_lossy(&refs.stdout)
        .lines()
        .any(|r| r.starts_with(&prefix));
    let mut result = Outgoing {
        report: Report::with_config(config),
        ..Outgoing::default()
    };
    let mut ranges = Vec::new();
    for update in updates {
        if zero(&update.local) {
            continue;
        }
        result.ref_updates += 1;
        let known = if zero(&update.remote) {
            false
        } else {
            command(root, routing)?
                .args(["cat-file", "-e", &format!("{}^{{commit}}", update.remote)])
                .status()
                .map_err(|_| GitError::Failed)?
                .success()
        };
        result.report.coverage.approximate |= !zero(&update.remote) && !known;
        let mut args = vec![update.local];
        if tracking {
            // --not toggles all following arguments, including --remotes.
            args.push("--not".into());
            if known {
                args.push(update.remote);
            }
            args.push(format!("--remotes={remote}"));
        } else {
            result.full_history = true;
        }
        ranges.push(args);
    }
    enumerate(root, routing, ranges, &mut result.report)?;
    Ok(result)
}

pub fn scan_push(root: &Path, routing: bool, remote: Option<&str>) -> Result<Outgoing, GitError> {
    scan_push_with_config(root, routing, remote, &Config::default())
}
pub fn scan_push_with_config(
    root: &Path,
    routing: bool,
    remote: Option<&str>,
    config: &Config,
) -> Result<Outgoing, GitError> {
    probe(root, routing)?;
    let branch = command(root, routing)?
        .args(["symbolic-ref", "--quiet", "--short", "HEAD"])
        .output()
        .map_err(|_| GitError::Failed)?;
    if !branch.status.success() {
        return Err(GitError::NoBranch);
    }
    let branch = String::from_utf8(branch.stdout).map_err(|_| GitError::Failed)?;
    let branch = branch.trim();
    let local = resolve(root, routing, "HEAD")?.ok_or(GitError::NoBranch)?;
    let configured = command(root, routing)?
        .args(["config", "--get", &format!("branch.{branch}.remote")])
        .output()
        .map_err(|_| GitError::Failed)?;
    let configured = String::from_utf8(configured.stdout).map_err(|_| GitError::Failed)?;
    let remote = remote.unwrap_or_else(|| {
        if configured.trim().is_empty() {
            "origin"
        } else {
            configured.trim()
        }
    });
    let target = match resolve(root, routing, "@{push}")? {
        Some(oid) => Some(oid),
        None => resolve(root, routing, &format!("refs/remotes/{remote}/{branch}"))?,
    };
    scan_updates(
        root,
        routing,
        remote,
        vec![Update {
            local,
            remote: target.unwrap_or_else(|| "0".repeat(40)),
        }],
        config,
    )
}

pub fn scan_commits(root: &Path, routing: bool, range: &str) -> Result<Outgoing, GitError> {
    scan_commits_with_config(root, routing, range, &Config::default())
}
pub fn scan_commits_with_config(
    root: &Path,
    routing: bool,
    range: &str,
    config: &Config,
) -> Result<Outgoing, GitError> {
    let (a, b) = range.split_once("..").ok_or(GitError::InvalidRange)?;
    if a.is_empty() || b.is_empty() || a.contains("..") || b.contains("..") || b.starts_with('.') {
        return Err(GitError::InvalidRange);
    }
    probe(root, routing)?;
    let a = resolve(root, routing, a)?.ok_or(GitError::InvalidRange)?;
    let b = resolve(root, routing, b)?.ok_or(GitError::InvalidRange)?;
    let mut result = Outgoing {
        ref_updates: 1,
        commit_range: Some(range.into()),
        report: Report::with_config(config),
        ..Outgoing::default()
    };
    enumerate(
        root,
        routing,
        vec![vec![b, "--not".into(), a]],
        &mut result.report,
    )?;
    Ok(result)
}

fn enumerate(
    root: &Path,
    routing: bool,
    ranges: Vec<Vec<String>>,
    report: &mut Report,
) -> Result<(), GitError> {
    if ranges.is_empty() {
        return Ok(());
    }
    let help = command(root, routing)?
        .args(["rev-list", "-h"])
        .stderr(Stdio::piped())
        .output()
        .map_err(|_| GitError::Failed)?;
    let nul = [&help.stdout, &help.stderr].iter().any(|bytes| {
        String::from_utf8_lossy(bytes)
            .split_whitespace()
            .any(|word| word == "-z")
    });
    let mut seen = std::collections::HashSet::new();
    let mut objects = Vec::new();
    let mut aliases: std::collections::BTreeMap<String, std::collections::BTreeSet<String>> =
        std::collections::BTreeMap::new();
    for range in ranges {
        // Object traversal supplies only one path per object. Commit diffs preserve
        // every path's allow scope and config changes even when blobs are reused.
        let changes = command(root, routing)?
            .args([
                "log",
                "--raw",
                "--format=",
                "-z",
                "--no-abbrev",
                "--no-renames",
                "--root",
                "-m",
                "--no-ext-diff",
                "--no-textconv",
            ])
            .args(&range)
            .arg("--")
            .output()
            .map_err(|_| GitError::Failed)?;
        if !changes.status.success() {
            return Err(GitError::Failed);
        }
        let mut fields = changes.stdout.split(|b| *b == 0);
        while let Some(header) = fields.next() {
            let header = std::str::from_utf8(header)
                .map_err(|_| GitError::Failed)?
                .trim_start_matches('\n');
            if header.is_empty() {
                continue;
            }
            let path = fields.next().ok_or(GitError::Failed)?;
            let path = String::from_utf8_lossy(path).into_owned();
            report.outgoing_config_changed |= path == ".hushpush.toml";
            let parts: Vec<_> = header.split(' ').collect();
            if parts.len() != 5 || !parts[0].starts_with(':') || !valid_oid(parts[3]) {
                return Err(GitError::Failed);
            }
            if matches!(parts[4], "A" | "M" | "T") {
                aliases.entry(parts[3].into()).or_default().insert(path);
            }
        }
        let mut cmd = command(root, routing)?;
        cmd.args([
            "rev-list",
            "--objects",
            "--missing=print",
            "--filter=object:type=blob",
        ]);
        if nul {
            cmd.arg("-z");
        }
        let output = cmd
            .args(range)
            .arg("--")
            .output()
            .map_err(|_| GitError::Failed)?;
        if !output.status.success() {
            return Err(GitError::Failed);
        }
        for (oid, path, missing) in object_records(&output.stdout, nul)? {
            report.outgoing_config_changed |= path == ".hushpush.toml";
            if seen.insert(oid.clone()) {
                objects.push((oid, path, missing));
            }
        }
    }
    let mut sizes = Batch::new(root, routing, "--batch-check")?;
    let mut pending = Vec::new();
    for (oid, path, missing) in objects {
        if missing {
            skip(report, path, SkipReason::MissingObject);
            continue;
        }
        match sizes.query(&oid)? {
            None => skip(report, path, SkipReason::MissingObject),
            Some((kind, size)) if kind == "blob" => {
                if size
                    > report
                        .config
                        .limits
                        .image_bytes()
                        .max(report.config.limits.text_bytes())
                {
                    let reason = oversized_reason(root, routing, &oid, false)?;
                    skip(report, path, reason);
                } else {
                    pending.push((oid, path, size));
                }
            }
            Some(_) => {}
        }
    }
    sizes.finish()?;
    let mut contents = Batch::new(root, routing, "--batch")?;
    for (oid, path, size) in pending {
        let path = aliases
            .get(&oid)
            .and_then(|paths| paths.first())
            .cloned()
            .unwrap_or(path);
        match contents.query(&oid)? {
            None => {
                skip(report, path, SkipReason::MissingObject);
                continue;
            }
            Some((kind, actual)) if kind == "blob" && actual == size => {}
            _ => return Err(GitError::Failed),
        }
        let mut bytes = vec![0; size as usize];
        contents
            .output
            .read_exact(&mut bytes)
            .map_err(|_| GitError::Failed)?;
        let mut newline = [0];
        contents
            .output
            .read_exact(&mut newline)
            .map_err(|_| GitError::Failed)?;
        if newline != *b"\n" {
            return Err(GitError::Failed);
        }
        let start = report.findings.len();
        scan_bytes(
            report,
            path.clone(),
            &bytes,
            Origin {
                blob_oid: Some(&oid),
                symlink: false,
            },
            false,
        );
        if let Some(paths) = aliases.get(&oid) {
            let findings = report.findings[start..].to_vec();
            for alias in paths.iter().filter(|alias| **alias != path) {
                for finding in &findings {
                    let mut finding = finding.clone();
                    finding.path = alias.clone();
                    report.findings.push(finding);
                }
            }
        }
    }
    contents.finish()
}

// With older Git, newline-containing names are displayed lossily: rev-list truncates
// the path at the newline. Names are labels only and never used to open files.
fn object_records(raw: &[u8], nul: bool) -> Result<Vec<(String, String, bool)>, GitError> {
    let mut result: Vec<(String, String, bool)> = Vec::new();
    for field in raw.split(|b| *b == if nul { 0 } else { b'\n' }) {
        if field.is_empty() {
            continue;
        }
        if nul && field == b"missing=yes" {
            result.last_mut().ok_or(GitError::Failed)?.2 = true;
            continue;
        }
        if nul && field.starts_with(b"path=") {
            let last = result.last_mut().ok_or(GitError::Failed)?;
            last.1 = String::from_utf8_lossy(&field[5..]).into_owned();
            continue;
        }
        let (missing, field) = if field[0] == b'?' {
            (true, &field[1..])
        } else {
            (false, field)
        };
        let (oid, path) = if !nul {
            match field.iter().position(|b| *b == b' ') {
                Some(i) => (&field[..i], Some(&field[i + 1..])),
                None => (field, None),
            }
        } else {
            (field, None)
        };
        let oid = std::str::from_utf8(oid).map_err(|_| GitError::Failed)?;
        if !valid_oid(oid) {
            return Err(GitError::Failed);
        }
        result.push((
            oid.into(),
            path.map(|p| String::from_utf8_lossy(p).into_owned())
                .unwrap_or_else(|| oid.into()),
            missing,
        ));
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn object_protocols_preserve_labels_and_missing_status() {
        let oid = "a".repeat(40);
        let raw = format!("{oid}\0path=line\nfile\0missing=yes\0");
        let records = object_records(raw.as_bytes(), true).unwrap();
        assert_eq!(records, vec![(oid.clone(), "line\nfile".into(), true)]);
        let raw = format!("?{oid}\n{oid} first path\n");
        let records = object_records(raw.as_bytes(), false).unwrap();
        assert_eq!(
            records,
            vec![
                (oid.clone(), oid.clone(), true),
                (oid, "first path".into(), false)
            ]
        );
    }

    #[test]
    fn ownership_failure_is_a_redacted_configuration_error() {
        let error = repository_error(
            b"fatal: detected dubious ownership in repository at '/synthetic-private-path'\n",
        );
        assert_eq!(error.exit_code(), ExitCode::UsageOrConfigError);
        assert_eq!(
            error.to_string(),
            "repository ownership check failed (see git safe.directory)"
        );
    }
}
