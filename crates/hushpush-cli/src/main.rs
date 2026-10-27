use std::path::PathBuf;

use clap::{Parser, Subcommand};
use hushpush_core::ExitCode;

mod report_file;

#[derive(Parser)]
#[command(name = "hushpush", version, about = "Local privacy checks (pre-alpha)")]
struct Cli {
    /// Directory containing the pinned local OCR models.
    #[arg(long, global = true, value_name = "DIR")]
    models: Option<PathBuf>,
    /// Read configuration from this working-tree file.
    #[arg(long, global = true, value_name = "FILE")]
    config: Option<PathBuf>,
    /// Override the in-scope coverage failure policy.
    #[arg(long, global = true, value_parser = ["block", "warn"])]
    on_incomplete: Option<String>,
    /// Include reserved email domains and fictional phone numbers (for synthetic test data).
    #[arg(
        long = "include-reserved",
        alias = "include-reserved-domains",
        global = true
    )]
    include_reserved_domains: bool,
    /// Disable user-global patterns.
    #[arg(long, global = true)]
    no_user_patterns: bool,
    /// Disable scope and value allowlists for this run.
    #[arg(long, global = true)]
    no_allow: bool,
    /// Write a JSON v1 report to FILE, or '-' for stdout.
    #[arg(long, global = true, value_name = "FILE")]
    json: Option<PathBuf>,
    /// Run as if started in DIR, ignoring inherited Git repository and index routing.
    #[arg(short = 'C', global = true, value_name = "DIR")]
    directory: Option<PathBuf>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Allow {
        #[command(subcommand)]
        command: AllowCommand,
    },
    Forget {
        #[arg(long)]
        yes: bool,
    },
    Scan {
        #[command(subcommand)]
        command: ScanCommand,
    },
    Redact {
        path: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    Hook {
        #[command(subcommand)]
        command: HookCommand,
    },
    #[cfg(debug_assertions)]
    #[command(hide = true)]
    DebugPanic,
}

#[derive(Subcommand)]
enum AllowCommand {
    Value {
        fp: String,
        #[arg(long)]
        reason: String,
        #[arg(long)]
        expires: Option<String>,
    },
    List,
}

#[derive(Subcommand)]
enum ScanCommand {
    Dir {
        path: PathBuf,
    },
    Staged,
    Push {
        #[arg(long)]
        remote: Option<String>,
    },
    Commits {
        range: String,
    },
}

#[derive(Subcommand)]
enum HookCommand {
    PrePush { remote: String, url: String },
    Print,
}

fn install_panic_hook() {
    std::panic::set_hook(Box::new(|info| {
        // The OCR boundary catches failures and reports incomplete coverage without panic payloads.
        if std::thread::current().name() == Some("hushpush-ocr") {
            return;
        }
        use std::io::Write;

        let mut stderr = std::io::stderr().lock();
        let _ = writeln!(stderr, "hushpush: internal error (exit 4)");
        if std::env::var_os("HUSHPUSH_DEBUG").is_some_and(|value| value == "1")
            && let Some(location) = info.location()
        {
            let _ = writeln!(stderr, "at {location}");
        }
        let _ = stderr.flush();
        std::process::exit(ExitCode::InternalError as i32);
    }));
}

fn main() -> std::process::ExitCode {
    install_panic_hook();

    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(error) => {
            let outcome = if error.use_stderr() {
                ExitCode::UsageOrConfigError
            } else {
                ExitCode::NoBlockingFindings
            };
            if error.print().is_err() {
                return ExitCode::InternalError.into();
            }
            return outcome.into();
        }
    };

    if std::env::var_os("HUSHPUSH_MODELS").is_none()
        && std::env::var_os("LEAKLENS_MODELS").is_some()
    {
        eprintln!("hushpush: LEAKLENS_MODELS is ignored; use HUSHPUSH_MODELS");
    }

    if let Some(models) = cli.models {
        let _ = hushpush_core::ocr::configure_models(models);
    }
    let inherit_git_routing = cli.directory.is_none();
    if let Some(directory) = cli.directory
        && std::env::set_current_dir(directory).is_err()
    {
        eprintln!("hushpush: directory is unavailable");
        return ExitCode::UsageOrConfigError.into();
    }

    if cli.json.is_some()
        && !matches!(
            &cli.command,
            Command::Scan { .. }
                | Command::Hook {
                    command: HookCommand::PrePush { .. }
                }
        )
    {
        eprintln!("hushpush: --json requires a scan or hook pre-push");
        return ExitCode::UsageOrConfigError.into();
    }

    if let Command::Forget { yes } = &cli.command {
        if !yes {
            eprintln!(
                "hushpush: forget deletes the repository key and value allows; pass --yes to confirm"
            );
            return ExitCode::UsageOrConfigError.into();
        }
        return match hushpush_core::allow::LocalStore::forget(
            std::path::Path::new("."),
            inherit_git_routing,
        ) {
            Ok(()) => ExitCode::NoBlockingFindings.into(),
            Err(error) => {
                eprintln!("hushpush: {error}");
                ExitCode::UsageOrConfigError.into()
            }
        };
    }

    let mut config = match hushpush_core::config::Config::load(
        cli.config.as_deref(),
        std::path::Path::new("."),
        inherit_git_routing,
    ) {
        Ok(config) => config,
        Err(error) => {
            eprintln!("hushpush: {error}");
            return ExitCode::UsageOrConfigError.into();
        }
    };
    if config.deprecated_clipboard_loose {
        eprintln!(
            "hushpush: detectors.clipboard_loose is deprecated; use detectors.clipboard_field"
        );
    }

    if cli.include_reserved_domains {
        config.detectors.email.include_reserved_domains = true;
        config.detectors.phone.include_reserved = true;
    }
    if let Some(policy) = cli.on_incomplete {
        config.policy.on_incomplete = if policy == "block" {
            hushpush_core::config::PolicyAction::Block
        } else {
            hushpush_core::config::PolicyAction::Warn
        };
    }

    if !cli.no_user_patterns {
        let root = hushpush_core::source::git::working_tree_root(
            std::path::Path::new("."),
            inherit_git_routing,
        )
        .unwrap_or_else(|| std::path::PathBuf::from("."));
        match hushpush_core::patterns::load(&root) {
            Ok(loaded) => {
                config.user_patterns = loaded.patterns;
                for warning in loaded.warnings {
                    eprintln!("hushpush: {warning}");
                }
            }
            Err(error) => {
                eprintln!("hushpush: {error}");
                return ExitCode::UsageOrConfigError.into();
            }
        }
    }

    let needs_store = !matches!(
        &cli.command,
        Command::Hook {
            command: HookCommand::Print
        }
    );
    let mut store = match if needs_store {
        hushpush_core::allow::LocalStore::discover(std::path::Path::new("."), inherit_git_routing)
    } else {
        Ok(None)
    } {
        Ok(store) => store,
        Err(error) => {
            eprintln!("hushpush: {error}");
            return ExitCode::UsageOrConfigError.into();
        }
    };

    match cli.command {
        Command::Forget { .. } => unreachable!(),
        Command::Allow { command } => match command {
            AllowCommand::Value {
                fp,
                reason,
                expires,
            } => {
                use hushpush_core::detect::Detector;
                if config
                    .user_patterns
                    .iter()
                    .any(|p| !p.find(&reason).is_empty())
                {
                    eprintln!("hushpush: invalid local allow key: reason");
                    return ExitCode::UsageOrConfigError.into();
                }
                let result = store
                    .as_mut()
                    .ok_or(hushpush_core::allow::AllowError::NotRepository)
                    .and_then(|store| store.append(&fp, &reason, expires.as_deref()));
                match result {
                    Ok(()) => ExitCode::NoBlockingFindings.into(),
                    Err(error) => {
                        eprintln!("hushpush: {error}");
                        ExitCode::UsageOrConfigError.into()
                    }
                }
            }
            AllowCommand::List => {
                if list_allows(&config, store.as_ref()).is_err() {
                    ExitCode::InternalError.into()
                } else {
                    ExitCode::NoBlockingFindings.into()
                }
            }
        },
        Command::Scan { command } => match command {
            ScanCommand::Dir { path } => {
                match hushpush_core::source::scan_dir_with_config(&path, &config) {
                    Ok(mut report) => {
                        report.apply_allowlists(store.as_ref(), cli.no_allow);
                        emit(&report, "dir", false, cli.json.as_deref(), |out| {
                            report.write_terminal(out)
                        })
                    }
                    Err(error) => {
                        eprintln!("hushpush: {error}");
                        ExitCode::UsageOrConfigError.into()
                    }
                }
            }
            ScanCommand::Staged => {
                match hushpush_core::source::git::scan_staged_with_config(
                    std::path::Path::new("."),
                    inherit_git_routing,
                    &config,
                ) {
                    Ok(mut report) => {
                        report.apply_allowlists(store.as_ref(), cli.no_allow);
                        emit(&report, "staged", false, cli.json.as_deref(), |out| {
                            report.write_terminal_staged(out)
                        })
                    }
                    Err(error) => {
                        eprintln!("hushpush: {error}");
                        error.exit_code().into()
                    }
                }
            }
            ScanCommand::Push { remote } => outgoing(
                hushpush_core::source::git::scan_push_with_config(
                    std::path::Path::new("."),
                    inherit_git_routing,
                    remote.as_deref(),
                    &config,
                ),
                store.as_ref(),
                cli.no_allow,
                cli.json.as_deref(),
                "push",
            ),
            ScanCommand::Commits { range } => outgoing(
                hushpush_core::source::git::scan_commits_with_config(
                    std::path::Path::new("."),
                    inherit_git_routing,
                    &range,
                    &config,
                ),
                store.as_ref(),
                cli.no_allow,
                cli.json.as_deref(),
                "commits",
            ),
        },
        Command::Redact { path, out } => {
            match hushpush_core::redact::redact_with_config(&path, &out, &config) {
                Ok(mut report) => {
                    report.audit.apply_allowlists(store.as_ref(), cli.no_allow);
                    if report
                        .write_terminal(std::io::stdout().lock(), &out)
                        .is_err()
                    {
                        ExitCode::InternalError.into()
                    } else {
                        report.exit_code().into()
                    }
                }
                Err(error) => {
                    eprintln!("hushpush: {error}");
                    ExitCode::UsageOrConfigError.into()
                }
            }
        }
        Command::Hook { command } => match command {
            HookCommand::PrePush { remote, url: _ } => {
                use std::io::Read;
                let mut input = String::new();
                if std::io::stdin().read_to_string(&mut input).is_err() {
                    eprintln!("hushpush: invalid pre-push input");
                    return ExitCode::UsageOrConfigError.into();
                }
                outgoing(
                    hushpush_core::source::git::scan_pre_push_with_config(
                        std::path::Path::new("."),
                        inherit_git_routing,
                        &remote,
                        &input,
                        &config,
                    ),
                    store.as_ref(),
                    cli.no_allow,
                    cli.json.as_deref(),
                    "push",
                )
            }
            HookCommand::Print => {
                use std::io::Write;
                return if std::io::stdout().lock().write_all(
                    b"#!/bin/sh\n# Install manually as .git/hooks/pre-push and chmod +x it.\n# git push --no-verify bypasses this check.\nexec hushpush hook pre-push \"$@\"\n"
                ).is_ok() { ExitCode::NoBlockingFindings.into() } else { ExitCode::InternalError.into() };
            }
        },
        #[cfg(debug_assertions)]
        Command::DebugPanic => panic!("synthetic-panic-payload-must-stay-hidden"),
    }
}

fn outgoing(
    result: Result<hushpush_core::source::git::Outgoing, hushpush_core::source::git::GitError>,
    store: Option<&hushpush_core::allow::LocalStore>,
    disabled: bool,
    json: Option<&std::path::Path>,
    mode: &str,
) -> std::process::ExitCode {
    match result {
        Ok(mut result) => {
            result.report.apply_allowlists(store, disabled);
            emit(&result.report, mode, result.full_history, json, |out| {
                result.write_terminal(out)
            })
        }
        Err(error) => {
            eprintln!("hushpush: {error}");
            error.exit_code().into()
        }
    }
}

fn emit(
    report: &hushpush_core::report::Report,
    mode: &str,
    full_history: bool,
    json: Option<&std::path::Path>,
    human: impl FnOnce(&mut dyn std::io::Write) -> std::io::Result<()>,
) -> std::process::ExitCode {
    let stdout_json = json == Some(std::path::Path::new("-"));
    let human_result = if stdout_json {
        human(&mut std::io::stderr().lock())
    } else {
        human(&mut std::io::stdout().lock())
    };
    let json_result = match json {
        Some(_) if stdout_json => report
            .write_json(std::io::stdout().lock(), mode, full_history)
            .map_err(|_| ExitCode::InternalError),
        Some(path) => report_file::write(path, |out| report.write_json(out, mode, full_history)),
        None => Ok(()),
    };
    if let Err(code) = json_result {
        eprintln!(
            "hushpush: {}",
            if code == ExitCode::UsageOrConfigError {
                "report target must not be a symlink or directory"
            } else {
                "could not write JSON report"
            }
        );
        return code.into();
    }
    if human_result.is_err() {
        ExitCode::InternalError.into()
    } else {
        report.exit_code().into()
    }
}

fn list_allows(
    config: &hushpush_core::config::Config,
    store: Option<&hushpush_core::allow::LocalStore>,
) -> std::io::Result<()> {
    use std::io::Write;
    let today = hushpush_core::allow::today();
    let mut out = std::io::stdout().lock();
    for rule in &config.allow {
        writeln!(
            out,
            "allow:{} reason={} expires={} status={}",
            rule.id,
            config.masked_path(&rule.reason),
            rule.expires
                .map(|date| date.to_string())
                .unwrap_or_else(|| "never".into()),
            if rule.expired(today) {
                "expired"
            } else {
                "active"
            }
        )?;
    }
    if let Some(store) = store {
        for rule in store.values() {
            writeln!(
                out,
                "value:{} reason={} expires={} status={}",
                rule.fp,
                config.masked_path(&rule.reason),
                rule.expires
                    .map(|date| date.to_string())
                    .unwrap_or_else(|| "never".into()),
                if rule.expired() { "expired" } else { "active" }
            )?;
        }
    } else {
        writeln!(
            out,
            "Outside a Git repository: fingerprints and value allows are unavailable."
        )?;
    }
    Ok(())
}
