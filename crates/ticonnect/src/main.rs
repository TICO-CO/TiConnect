#![forbid(unsafe_code)]

use std::ffi::OsString;

use anyhow::Result;

const USAGE: &str = "Usage: ticonnect <command> [args...]

Commands:
  daemon    Run the TiConnect background daemon
  ui        Open the GTK4 application (default)
  overlay   Run the corner overlay agent
  cli       Run the command-line client
  help      Print this help";

fn main() -> Result<()> {
    let mut args = std::env::args_os();
    let program = args
        .next()
        .unwrap_or_else(|| OsString::from("ticonnect"))
        .to_string_lossy()
        .into_owned();
    let rest: Vec<OsString> = args.collect();

    let Some(command) = rest.first() else {
        return ticonnect_ui::run();
    };

    match command.to_str() {
        Some("daemon") => {
            runtime()?.block_on(ticonnect_runtime::run_daemon())?;
            Ok(())
        }
        Some("ui") => ticonnect_ui::run(),
        Some("overlay") => ticonnect_overlay::run(),
        Some("cli") => {
            runtime()?.block_on(ticonnect_cli::entry::run(
                std::iter::once(OsString::from("ticonnect-cli"))
                    .chain(rest.iter().skip(1).cloned()),
            ))?;
            Ok(())
        }
        Some("help" | "-h" | "--help") => {
            println!("{USAGE}");
            Ok(())
        }
        Some(other) => {
            eprintln!("{program}: unknown command: {other}");
            eprintln!("{USAGE}");
            std::process::exit(2);
        }
        None => {
            eprintln!("{program}: command is not valid UTF-8");
            std::process::exit(2);
        }
    }
}

fn runtime() -> Result<tokio::runtime::Runtime> {
    Ok(tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?)
}
