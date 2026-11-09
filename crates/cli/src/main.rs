#![forbid(unsafe_code)]

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use ticonnect_cli::client::DaemonProxy;
use ticonnect_cli::output::{status_human, status_json};

#[derive(Debug, Parser)]
#[command(
    name = "ticonnect-cli",
    version,
    about = "TiConnect command-line client"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Show daemon status.
    Status {
        /// Print machine-readable JSON.
        #[arg(long)]
        json: bool,
    },
    /// Print the daemon version.
    Version,
    /// Check that the daemon is reachable.
    Ping,
    /// Ask the daemon to stop.
    Quit,
}

#[tokio::main]
async fn main() -> Result<()> {
    let settings = ticonnect_settings::Settings::load().context("failed to load settings")?;
    let log_settings = ticonnect_logging::LogSettings::new(
        settings.log_level(),
        settings.log_file(),
        settings.log_format(),
    )
    .context("invalid logging settings")?;
    ticonnect_logging::Logging::init(&log_settings).context("failed to initialize logging")?;

    let cli = Cli::parse();
    run(cli.command).await
}

async fn run(command: Command) -> Result<()> {
    match command {
        Command::Status { json } => {
            let proxy = connect().await?;
            let version = proxy.get_version().await.context("get daemon version")?;
            let status = proxy.get_status().await.context("get daemon status")?;
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&status_json(&version, &status))
                        .context("serialize status")?
                );
            } else {
                print!("{}", status_human(&version, &status));
            }
        }
        Command::Version => {
            let proxy = connect().await?;
            println!(
                "{}",
                proxy.get_version().await.context("get daemon version")?
            );
        }
        Command::Ping => {
            let proxy = connect().await?;
            println!("{}", proxy.ping().await.context("ping daemon")?);
        }
        Command::Quit => {
            let proxy = connect().await?;
            proxy.shutdown().await.context("shut down daemon")?;
            println!("daemon stopping");
        }
    }
    Ok(())
}

async fn connect() -> Result<DaemonProxy<'static>> {
    let connection = zbus::Connection::session()
        .await
        .context("connect to the session bus")?;
    DaemonProxy::new(&connection)
        .await
        .context("create daemon proxy")
}
