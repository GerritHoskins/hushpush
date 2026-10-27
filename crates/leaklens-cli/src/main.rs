use std::path::PathBuf;

use clap::{Parser, Subcommand};
use leaklens_core::ExitCode;

#[derive(Parser)]
#[command(name = "leaklens", version, about = "Local privacy checks (pre-alpha)")]
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
    /// Include reserved email domains (for synthetic test data).
    #[arg(long, global = true)]
    include_reserved_domains: bool,
    /// Disable scope and value allowlists for this run.
    #[arg(long, global = true)]
    no_allow: bool,
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
        if std::thread::current().name() == Some("leaklens-ocr") {
            return;
        }
        use std::io::Write;

        let mut stderr = std::io::stderr().lock();
        let _ = writeln!(stderr, "leaklens: internal error (exit 4)");
        if std::env::var_os("LEAKLENS_DEBUG").is_some_and(|value| value == "1")
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

    if let Some(models) = cli.models {
        let _ = leaklens_core::ocr::configure_models(models);
    }
    let inherit_git_routing = cli.directory.is_none();
    if let Some(directory) = cli.directory
        && std::env::set_current_dir(directory).is_err()
    {
        eprintln!("leaklens: directory is unavailable");
        return ExitCode::UsageOrConfigError.into();
    }

    if let Command::Forget { yes } = &cli.command {
        if !yes {
            eprintln!(
                "leaklens: forget deletes the repository key and value allows; pass --yes to confirm"
            );
            return ExitCode::UsageOrConfigError.into();
        }
        return match leaklens_core::allow::LocalStore::forget(
            std::path::Path::new("."),
            inherit_git_routing,
        ) {
            Ok(()) => ExitCode::NoBlockingFindings.into(),
            Err(error) => {
                eprintln!("leaklens: {error}");
                ExitCode::UsageOrConfigError.into()
            }
        };
    }

    let mut config = match leaklens_core::config::Config::load(
        cli.config.as_deref(),
        std::path::Path::new("."),
        inherit_git_routing,
    ) {
        Ok(config) => config,
        Err(error) => {
            eprintln!("leaklens: {error}");
            return ExitCode::UsageOrConfigError.into();
        }
    };
    if cli.include_reserved_domains {
        config.detectors.email.include_reserved_domains = true;
    }
    if let Some(policy) = cli.on_incomplete {
        config.policy.on_incomplete = if policy == "block" {
            leaklens_core::config::PolicyAction::Block
        } else {
            leaklens_core::config::PolicyAction::Warn
        };
    }

    let needs_store = !matches!(
        &cli.command,
        Command::Hook {
            command: HookCommand::Print
        }
    );
    let mut store = match if needs_store {
        leaklens_core::allow::LocalStore::discover(std::path::Path::new("."), inherit_git_routing)
    } else {
        Ok(None)
    } {
        Ok(store) => store,
        Err(error) => {
            eprintln!("leaklens: {error}");
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
                let result = store
                    .as_mut()
                    .ok_or(leaklens_core::allow::AllowError::NotRepository)
                    .and_then(|store| store.append(&fp, &reason, expires.as_deref()));
                match result {
                    Ok(()) => ExitCode::NoBlockingFindings.into(),
                    Err(error) => {
                        eprintln!("leaklens: {error}");
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
                match leaklens_core::source::scan_dir_with_config(&path, &config) {
                    Ok(mut report) => {
                        report.apply_allowlists(store.as_ref(), cli.no_allow);
                        if report.write_terminal(std::io::stdout().lock()).is_err() {
                            ExitCode::InternalError.into()
                        } else {
                            report.exit_code().into()
                        }
                    }
                    Err(error) => {
                        eprintln!("leaklens: {error}");
                        ExitCode::UsageOrConfigError.into()
                    }
                }
            }
            ScanCommand::Staged => {
                match leaklens_core::source::git::scan_staged_with_config(
                    std::path::Path::new("."),
                    inherit_git_routing,
                    &config,
                ) {
                    Ok(mut report) => {
                        report.apply_allowlists(store.as_ref(), cli.no_allow);
                        if report
                            .write_terminal_staged(std::io::stdout().lock())
                            .is_err()
                        {
                            ExitCode::InternalError.into()
                        } else {
                            report.exit_code().into()
                        }
                    }
                    Err(error) => {
                        eprintln!("leaklens: {error}");
                        error.exit_code().into()
                    }
                }
            }
            ScanCommand::Push { remote } => outgoing(
                leaklens_core::source::git::scan_push_with_config(
                    std::path::Path::new("."),
                    inherit_git_routing,
                    remote.as_deref(),
                    &config,
                ),
                store.as_ref(),
                cli.no_allow,
            ),
            ScanCommand::Commits { range } => outgoing(
                leaklens_core::source::git::scan_commits_with_config(
                    std::path::Path::new("."),
                    inherit_git_routing,
                    &range,
                    &config,
                ),
                store.as_ref(),
                cli.no_allow,
            ),
        },
        Command::Redact { path, out } => {
            match leaklens_core::redact::redact_with_config(&path, &out, &config) {
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
                    eprintln!("leaklens: {error}");
                    ExitCode::UsageOrConfigError.into()
                }
            }
        }
        Command::Hook { command } => match command {
            HookCommand::PrePush { remote, url: _ } => {
                use std::io::Read;
                let mut input = String::new();
                if std::io::stdin().read_to_string(&mut input).is_err() {
                    eprintln!("leaklens: invalid pre-push input");
                    return ExitCode::UsageOrConfigError.into();
                }
                outgoing(
                    leaklens_core::source::git::scan_pre_push_with_config(
                        std::path::Path::new("."),
                        inherit_git_routing,
                        &remote,
                        &input,
                        &config,
                    ),
                    store.as_ref(),
                    cli.no_allow,
                )
            }
            HookCommand::Print => {
                use std::io::Write;
                return if std::io::stdout().lock().write_all(
                    b"#!/bin/sh\n# Install manually as .git/hooks/pre-push and chmod +x it.\n# git push --no-verify bypasses this check.\nexec leaklens hook pre-push \"$@\"\n"
                ).is_ok() { ExitCode::NoBlockingFindings.into() } else { ExitCode::InternalError.into() };
            }
        },
        #[cfg(debug_assertions)]
        Command::DebugPanic => panic!("synthetic-panic-payload-must-stay-hidden"),
    }
}

fn outgoing(
    result: Result<leaklens_core::source::git::Outgoing, leaklens_core::source::git::GitError>,
    store: Option<&leaklens_core::allow::LocalStore>,
    disabled: bool,
) -> std::process::ExitCode {
    match result {
        Ok(mut result) => {
            result.report.apply_allowlists(store, disabled);
            if result.write_terminal(std::io::stdout().lock()).is_err() {
                ExitCode::InternalError.into()
            } else {
                result.report.exit_code().into()
            }
        }
        Err(error) => {
            eprintln!("leaklens: {error}");
            error.exit_code().into()
        }
    }
}

fn list_allows(
    config: &leaklens_core::config::Config,
    store: Option<&leaklens_core::allow::LocalStore>,
) -> std::io::Result<()> {
    use leaklens_core::report::sanitize_path;
    use std::io::Write;
    let today = leaklens_core::allow::today();
    let mut out = std::io::stdout().lock();
    for rule in &config.allow {
        writeln!(
            out,
            "allow:{} reason={} expires={} status={}",
            rule.id,
            sanitize_path(&rule.reason),
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
                sanitize_path(&rule.reason),
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
