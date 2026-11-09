#![forbid(unsafe_code)]

mod systems;

use anyhow::{Result, bail};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Write,
    Check,
}

fn main() -> Result<()> {
    match parse(std::env::args().skip(1))? {
        Command::Systems(mode) => systems::run(mode.into()),
        Command::Help => {
            print_help();
            Ok(())
        }
    }
}

enum Command {
    Systems(Mode),
    Help,
}

impl From<Mode> for systems::RunMode {
    fn from(mode: Mode) -> Self {
        match mode {
            Mode::Write => Self::Write,
            Mode::Check => Self::Check,
        }
    }
}

fn parse(mut args: impl Iterator<Item = String>) -> Result<Command> {
    let Some(command) = args.next() else {
        bail!("missing command\n\n{HELP}");
    };
    match command.as_str() {
        "help" | "--help" | "-h" => Ok(Command::Help),
        "systems" => {
            let mode = match args.next().as_deref() {
                None => Mode::Write,
                Some("--check") => Mode::Check,
                Some("--help" | "-h") => return Ok(Command::Help),
                Some(other) => bail!("unknown systems option: {other}\n\n{HELP}"),
            };
            if let Some(extra) = args.next() {
                bail!("unexpected argument: {extra}\n\n{HELP}");
            }
            Ok(Command::Systems(mode))
        }
        other => bail!("unknown xtask command: {other}\n\n{HELP}"),
    }
}

fn print_help() {
    println!("{HELP}");
}

const HELP: &str = "\
Usage: cargo xtask <COMMAND>

Commands:
  systems            Regenerate architecture docs
  systems --check    Validate graph and fail when generated docs are stale
  help               Show this help
";
