#![forbid(unsafe_code)]

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use futures_util::StreamExt;
use ticonnect_cli::client::DaemonProxy;
use ticonnect_cli::output::{candidate_line, status_human, status_json, variant_text};
use ticonnect_core::ipc;
use ticonnect_settings::Settings;

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
    /// Show this node's identity.
    Id {
        #[command(subcommand)]
        command: IdCommand,
    },
    /// Read or write settings.
    Settings {
        #[command(subcommand)]
        command: SettingsCommand,
    },
    /// Inspect known peers.
    Peers {
        #[command(subcommand)]
        command: PeersCommand,
    },
    /// Inspect discovery candidates.
    Discover {
        #[command(subcommand)]
        command: DiscoverCommand,
    },
}

#[derive(Debug, Subcommand)]
enum IdCommand {
    /// Print node id and fingerprint.
    Show,
}

#[derive(Debug, Subcommand)]
enum SettingsCommand {
    /// Print one setting.
    Get { key: String },
    /// Set one setting and save it.
    Set { key: String, value: String },
    /// List all known settings.
    List,
}

#[derive(Debug, Subcommand)]
enum PeersCommand {
    /// List known peers.
    List,
}

#[derive(Debug, Subcommand)]
enum DiscoverCommand {
    /// List discovered candidates.
    List,
    /// Print candidate found and lost events.
    Watch,
}

#[tokio::main]
async fn main() -> Result<()> {
    ticonnect_runtime::init().context("initialize runtime")?;

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
        Command::Id {
            command: IdCommand::Show,
        } => {
            let proxy = connect().await?;
            let identity = proxy.get_identity().await.context("get daemon identity")?;
            println!(
                "node_id: {}",
                identity_value(&identity, ipc::IDENTITY_NODE_ID)
            );
            println!(
                "fingerprint: {}",
                group_fingerprint(&identity_value(&identity, ipc::IDENTITY_FINGERPRINT))
            );
            println!(
                "display_name: {}",
                identity_value(&identity, ipc::IDENTITY_DISPLAY_NAME)
            );
        }
        Command::Settings { command } => run_settings(command)?,
        Command::Peers {
            command: PeersCommand::List,
        } => {
            let proxy = connect().await?;
            let peers = proxy.list_peers().await.context("list peers")?;
            if peers.is_empty() {
                println!("no peers");
            } else {
                for peer in peers {
                    println!(
                        "{} {} {} {}",
                        peer_value(&peer, ipc::PEER_ID),
                        peer_value(&peer, ipc::PEER_FINGERPRINT),
                        peer_value(&peer, ipc::PEER_POLICY),
                        peer_value(&peer, ipc::PEER_DISPLAY_NAME)
                    );
                }
            }
        }
        Command::Discover { command } => run_discover(command).await?,
    }
    Ok(())
}

async fn run_discover(command: DiscoverCommand) -> Result<()> {
    let proxy = connect().await?;
    match command {
        DiscoverCommand::List => {
            let candidates = proxy.list_candidates().await.context("list candidates")?;
            if candidates.is_empty() {
                println!("no candidates");
            } else {
                for candidate in candidates {
                    println!("{}", candidate_line(&candidate));
                }
            }
        }
        DiscoverCommand::Watch => {
            let mut found = proxy
                .receive_candidate_found()
                .await
                .context("subscribe to candidates")?;
            let mut lost = proxy
                .receive_candidate_lost()
                .await
                .context("subscribe to candidate loss")?;

            loop {
                tokio::select! {
                    event = found.next() => {
                        let Some(event) = event else {
                            break;
                        };
                        let candidate = event.args().context("read candidate event")?;
                        println!("found {}", candidate_line(&candidate.candidate));
                    }
                    event = lost.next() => {
                        let Some(event) = event else {
                            break;
                        };
                        let event = event.args().context("read candidate loss event")?;
                        println!("lost {}", event.fingerprint);
                    }
                    _ = tokio::signal::ctrl_c() => break,
                }
            }
        }
    }
    Ok(())
}

fn run_settings(command: SettingsCommand) -> Result<()> {
    let mut settings = Settings::load().context("load settings")?;
    match command {
        SettingsCommand::Get { key } => {
            let value = settings.get_setting(&key).context("get setting")?;
            println!("{}", value.render());
        }
        SettingsCommand::Set { key, value } => {
            settings.set_setting(&key, &value).context("set setting")?;
            settings.save().context("save settings")?;
            println!("{} = {}", key, value);
        }
        SettingsCommand::List => {
            for (key, value) in settings.list_settings().context("list settings")? {
                println!("{key} = {}", value.render());
            }
        }
    }
    Ok(())
}

fn group_fingerprint(value: &str) -> String {
    value
        .as_bytes()
        .chunks(4)
        .map(|chunk| std::str::from_utf8(chunk).unwrap_or_default())
        .collect::<Vec<_>>()
        .join(" ")
}

fn identity_value(
    identity: &std::collections::HashMap<String, zbus::OwnedVariant>,
    key: &str,
) -> String {
    identity
        .get(key)
        .map(variant_text)
        .unwrap_or_else(|| "unknown".to_owned())
}

fn peer_value(peer: &std::collections::HashMap<String, zbus::OwnedVariant>, key: &str) -> String {
    peer.get(key)
        .map(variant_text)
        .unwrap_or_else(|| "unknown".to_owned())
}

async fn connect() -> Result<DaemonProxy<'static>> {
    let connection = zbus::Connection::session()
        .await
        .context("connect to the session bus")?;
    DaemonProxy::new(&connection)
        .await
        .context("create daemon proxy")
}
