#![forbid(unsafe_code)]

pub mod install;

use std::ffi::OsString;
use std::fmt;

use anyhow::Result;

pub const HELP: &str = "\
Usage:
  ticonnect              Launch TiConnect
  ticonnect <command>    Use the command-line client
  ticonnect --version
  ticonnect --help

Install:
  ticonnect --install-user
  ticonnect --uninstall-user [--purge]";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    App,
    Ui,
    Overlay,
    Daemon,
    Cli(Vec<OsString>),
    Help,
    Version,
    InstallUser { force: bool },
    UninstallUser { purge: bool, force: bool },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError(String);

impl fmt::Display for ParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for ParseError {}

pub fn parse<I, T>(args: I) -> std::result::Result<Action, ParseError>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString>,
{
    let args: Vec<OsString> = args.into_iter().map(Into::into).collect();
    let Some(first) = args.first() else {
        return Ok(Action::App);
    };
    let rest = &args[1..];
    let Some(command) = first.to_str() else {
        return Ok(Action::Cli(args));
    };

    match command {
        "help" | "-h" | "--help" => Ok(Action::Help),
        "version" | "-V" | "--version" => Ok(Action::Version),
        "--install-user" => Ok(Action::InstallUser {
            force: parse_flags(rest, &["--force"])?,
        }),
        "--uninstall-user" => {
            let purge = rest.iter().any(|arg| arg == "--purge");
            let force = rest.iter().any(|arg| arg == "--force");
            for arg in rest {
                if arg != "--purge" && arg != "--force" {
                    return Err(unknown_flag(arg));
                }
            }
            Ok(Action::UninstallUser { purge, force })
        }
        "daemon" => Ok(Action::Daemon),
        "overlay" => Ok(Action::Overlay),
        "ui" => Ok(Action::Ui),
        "cli" => Ok(Action::Cli(args[1..].to_vec())),
        other if other.starts_with('-') => Err(ParseError(format!("unknown option: {other}"))),
        _ => Ok(Action::Cli(args)),
    }
}

fn parse_flags(args: &[OsString], allowed: &[&str]) -> std::result::Result<bool, ParseError> {
    let mut seen = false;
    for arg in args {
        let Some(value) = arg.to_str() else {
            return Err(unknown_flag(arg));
        };
        if !allowed.contains(&value) {
            return Err(unknown_flag(arg));
        }
        seen = true;
    }
    Ok(seen)
}

fn unknown_flag(arg: &OsString) -> ParseError {
    ParseError(format!("unknown option: {}", arg.to_string_lossy()))
}

pub fn run(action: Action) -> Result<()> {
    match action {
        Action::App | Action::Ui => {
            let started = std::time::Instant::now();
            if let Err(error) = ticonnect_runtime::ensure_daemon() {
                eprintln!("warning: {error}");
            }
            if std::env::var_os("TICONNECT_UI_TIMING").is_some() {
                eprintln!(
                    "ui-timing +{}ms ensure_daemon done",
                    started.elapsed().as_millis()
                );
            }
            ticonnect_ui::run()
        }
        Action::Overlay => ticonnect_overlay::run(),
        Action::Daemon => {
            runtime()?.block_on(ticonnect_runtime::run_daemon())?;
            Ok(())
        }
        Action::Cli(args) => {
            if args.first().is_none_or(|arg| arg != "settings")
                && let Err(error) = ticonnect_runtime::ensure_daemon()
            {
                eprintln!("warning: {error}");
            }
            runtime()?.block_on(ticonnect_cli::entry::run(
                std::iter::once(OsString::from("ticonnect")).chain(args),
            ))?;
            Ok(())
        }
        Action::Help => {
            println!("{HELP}");
            Ok(())
        }
        Action::Version => {
            println!("ticonnect {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        Action::InstallUser { force } => install::install(force),
        Action::UninstallUser { purge, force } => install::uninstall(purge, force),
    }
}

fn runtime() -> Result<tokio::runtime::Runtime> {
    Ok(tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?)
}
