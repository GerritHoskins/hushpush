use std::path::PathBuf;

use clap::{Parser, Subcommand};
use leaklens_core::ExitCode;

#[derive(Parser)]
#[command(name = "leaklens", version, about = "Local privacy checks (pre-alpha)")]
struct Cli {
    /// Directory containing the pinned local OCR models.
    #[arg(long, global = true, value_name = "DIR")]
    models: Option<PathBuf>,
    /// Run as if started in DIR, ignoring inherited Git repository and index routing.
    #[arg(short = 'C', global = true, value_name = "DIR")]
    directory: Option<PathBuf>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Scan {
        #[command(subcommand)]
        command: ScanCommand,
    },
    Redact {
        path: PathBuf,
        #[arg(long)]
        out: PathBuf,
        #[arg(long)]
        include_reserved_domains: bool,
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
enum ScanCommand {
    Dir {
        path: PathBuf,
        /// Include reserved email domains (for synthetic test data).
        #[arg(long)]
        include_reserved_domains: bool,
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

    match cli.command {
        Command::Scan { command } => match command {
            ScanCommand::Dir {
                path,
                include_reserved_domains,
            } => match leaklens_core::source::scan_dir(&path, include_reserved_domains) {
                Ok(report) => {
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
            },
            ScanCommand::Staged => {
                match leaklens_core::source::git::scan_staged_with_routing(
                    std::path::Path::new("."),
                    inherit_git_routing,
                ) {
                    Ok(report) => {
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
            ScanCommand::Push { remote } => outgoing(leaklens_core::source::git::scan_push(
                std::path::Path::new("."),
                inherit_git_routing,
                remote.as_deref(),
            )),
            ScanCommand::Commits { range } => outgoing(leaklens_core::source::git::scan_commits(
                std::path::Path::new("."),
                inherit_git_routing,
                &range,
            )),
        },
        Command::Redact {
            path,
            out,
            include_reserved_domains,
        } => match leaklens_core::redact::redact(&path, &out, include_reserved_domains) {
            Ok(report) => {
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
        },
        Command::Hook { command } => match command {
            HookCommand::PrePush { remote, url: _ } => {
                use std::io::Read;
                let mut input = String::new();
                if std::io::stdin().read_to_string(&mut input).is_err() {
                    eprintln!("leaklens: invalid pre-push input");
                    return ExitCode::UsageOrConfigError.into();
                }
                outgoing(leaklens_core::source::git::scan_pre_push(
                    std::path::Path::new("."),
                    inherit_git_routing,
                    &remote,
                    &input,
                ))
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
) -> std::process::ExitCode {
    match result {
        Ok(result) => {
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
