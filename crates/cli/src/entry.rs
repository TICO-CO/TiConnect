use std::ffi::OsString;

use crate::output::{candidate_line, status_human, status_json, variant_text};
use anyhow::{Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use futures_util::StreamExt;
use ticonnect_core::id::NodeId;
use ticonnect_core::ipc;
use ticonnect_core::ipc::client::DaemonProxy;
use ticonnect_core::message::conversation_id;
use ticonnect_settings::Settings;

#[derive(Debug, Parser)]
#[command(name = "ticonnect", version, about = "TiConnect command-line client")]
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
    /// Inspect known users.
    Users {
        #[command(subcommand)]
        command: UsersCommand,
    },
    /// Inspect discovery candidates.
    Discover {
        #[command(subcommand)]
        command: DiscoverCommand,
    },
    /// Send a text message to a trusted user.
    Send { user_id: String, body: String },
    /// Send a file attachment to a trusted user.
    SendFile {
        user_id: String,
        path: String,
        #[arg(long, default_value = "")]
        mime: String,
    },
    /// Save a received attachment to a destination path.
    Download { hash: String, destination: String },
    /// Show or cancel an attachment transfer.
    Transfer {
        #[command(subcommand)]
        command: TransferCommand,
    },
    /// List message history for a user.
    Messages {
        user_id: String,
        #[arg(long, default_value_t = 50)]
        limit: u32,
        #[arg(long, default_value_t = 0)]
        offset: u32,
    },
    /// Mark inbound messages as read.
    MarkRead {
        user_id: String,
        message_ids: Vec<String>,
    },
    /// Read or change user policy and DND.
    Policy {
        #[command(subcommand)]
        command: PolicyCommand,
    },
    /// Control a voice call.
    Call {
        #[command(subcommand)]
        command: CallCommand,
    },
    /// Control a remote desktop session.
    Remote {
        #[command(subcommand)]
        command: RemoteCommand,
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
enum UsersCommand {
    /// List known users.
    List,
    /// Forget and block a trusted user.
    Forget { user_id: String },
    /// List blocked users.
    Blocked,
    /// Unblock a user fingerprint.
    Unblock { fingerprint: String },
}

#[derive(Debug, Subcommand)]
enum DiscoverCommand {
    /// List discovered candidates.
    List,
    /// Print candidate found and lost events.
    Watch,
}

#[derive(Debug, Subcommand)]
enum TransferCommand {
    /// Show transfer state for one attachment hash.
    Status { hash: String },
    /// Cancel a transfer in progress.
    Cancel { hash: String },
}

#[derive(Debug, Subcommand)]
enum CallCommand {
    /// Start an outgoing call.
    Start { user_id: String },
    /// Accept an incoming call.
    Accept { call_id: String },
    /// Decline an incoming call.
    Decline { call_id: String },
    /// Hang up an active call.
    Hangup { call_id: String },
    /// Show the current call state.
    Status,
    /// Show audio frame counters.
    Metrics,
}

#[derive(Debug, Subcommand)]
enum RemoteCommand {
    /// Request a remote desktop session.
    Request { user_id: String },
    /// Consent to a remote desktop request.
    Accept {
        session_id: String,
        #[arg(default_value = "")]
        pin: String,
    },
    /// Deny a remote session request.
    Deny { session_id: String },
    /// End or kill a remote session.
    End { session_id: String },
    /// Show the current remote session.
    Status,
}

#[derive(Debug, Subcommand)]
enum PolicyCommand {
    /// Set policy for one user.
    Set { user_id: String, policy: String },
    /// Toggle global do-not-disturb mode.
    Dnd { state: DndState },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum DndState {
    On,
    Off,
}

pub async fn run<I, T>(args: I) -> Result<()>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    ticonnect_runtime::init().context("initialize runtime")?;

    let cli = Cli::parse_from(args);
    run_command(cli.command).await
}

async fn run_command(command: Command) -> Result<()> {
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
        Command::Users { command } => run_users(command).await?,
        Command::Discover { command } => run_discover(command).await?,
        Command::Send { user_id, body } => {
            let proxy = connect().await?;
            let message_id = proxy
                .send_message(&user_id, &body)
                .await
                .context("send message")?;
            println!("{message_id}");
        }
        Command::SendFile {
            user_id,
            path,
            mime,
        } => {
            let proxy = connect().await?;
            let message_id = proxy
                .send_attachment(&user_id, &path, &mime)
                .await
                .context("send attachment")?;
            println!("{message_id}");
        }
        Command::Download { hash, destination } => {
            let proxy = connect().await?;
            proxy
                .save_attachment(&hash, &destination)
                .await
                .context("save attachment")?;
            println!("{destination}");
        }
        Command::Transfer { command } => {
            let proxy = connect().await?;
            match command {
                TransferCommand::Status { hash } => {
                    let state = proxy
                        .get_transfer_state(&hash)
                        .await
                        .context("get transfer state")?;
                    for key in [
                        ipc::TRANSFER_HASH,
                        ipc::TRANSFER_USER_ID,
                        ipc::TRANSFER_NAME,
                        ipc::TRANSFER_SIZE,
                        ipc::TRANSFER_BYTES,
                        ipc::TRANSFER_PHASE,
                        ipc::TRANSFER_REASON,
                    ] {
                        println!("{key}: {}", message_value(&state, key));
                    }
                }
                TransferCommand::Cancel { hash } => {
                    proxy
                        .cancel_transfer(&hash)
                        .await
                        .context("cancel transfer")?;
                    println!("cancelled");
                }
            }
        }
        Command::Messages {
            user_id,
            limit,
            offset,
        } => {
            let proxy = connect().await?;
            let conversation_id = derive_conversation(&proxy, &user_id).await?;
            let messages = proxy
                .list_messages(&conversation_id, limit, offset)
                .await
                .context("list messages")?;
            if messages.is_empty() {
                println!("no messages");
            } else {
                for message in messages {
                    println!(
                        "{} {} {} {} {}",
                        message_value(&message, ipc::MESSAGE_ID),
                        message_value(&message, ipc::MESSAGE_STATE),
                        message_value(&message, ipc::MESSAGE_SENT_AT),
                        message_value(&message, ipc::MESSAGE_SENDER_ID),
                        attachment_label(&message),
                    );
                }
            }
        }
        Command::MarkRead {
            user_id,
            message_ids,
        } => {
            let proxy = connect().await?;
            let conversation_id = derive_conversation(&proxy, &user_id).await?;
            let ids: Vec<&str> = message_ids.iter().map(String::as_str).collect();
            proxy
                .mark_read(&conversation_id, &ids)
                .await
                .context("mark read")?;
            println!("marked read");
        }
        Command::Call { command } => {
            let proxy = connect().await?;
            match command {
                CallCommand::Start { user_id } => {
                    println!(
                        "{}",
                        proxy.start_call(&user_id).await.context("start call")?
                    );
                }
                CallCommand::Accept { call_id } => {
                    proxy.accept_call(&call_id).await.context("accept call")?;
                    println!("accepted");
                }
                CallCommand::Decline { call_id } => {
                    proxy.decline_call(&call_id).await.context("decline call")?;
                    println!("declined");
                }
                CallCommand::Hangup { call_id } => {
                    proxy.hangup_call(&call_id).await.context("hangup call")?;
                    println!("hung up");
                }
                CallCommand::Metrics => {
                    let metrics = proxy.get_call_metrics().await.context("get call metrics")?;
                    for key in [
                        ipc::CALL_FRAMES_SENT,
                        ipc::CALL_FRAMES_RECEIVED,
                        ipc::CALL_PACKETS_LOST,
                        ipc::CALL_BYTES_SENT,
                    ] {
                        println!("{key}: {}", message_value(&metrics, key));
                    }
                }
                CallCommand::Status => {
                    let state = proxy.get_call_state().await.context("get call state")?;
                    if state.is_empty() {
                        println!("idle");
                    } else {
                        for key in [
                            ipc::CALL_ID,
                            ipc::CALL_USER_ID,
                            ipc::CALL_DIRECTION,
                            ipc::CALL_STATE,
                            ipc::CALL_STARTED_AT,
                            ipc::CALL_CONNECTED_AT,
                        ] {
                            println!("{key}: {}", message_value(&state, key));
                        }
                    }
                }
            }
        }
        Command::Remote { command } => {
            let proxy = connect().await?;
            match command {
                RemoteCommand::Request { user_id } => {
                    let session = proxy
                        .request_remote_session(&user_id)
                        .await
                        .context("request remote session")?;
                    for key in [
                        ipc::REMOTE_SESSION_ID,
                        ipc::REMOTE_USER_ID,
                        ipc::REMOTE_ROLE,
                        ipc::REMOTE_STATE,
                    ] {
                        println!("{key}: {}", message_value(&session, key));
                    }
                }
                RemoteCommand::Accept { session_id, pin } => {
                    proxy
                        .accept_remote_session(&session_id, &pin)
                        .await
                        .context("accept remote session")?;
                    println!("accepted");
                }
                RemoteCommand::Deny { session_id } => {
                    proxy
                        .deny_remote_session(&session_id)
                        .await
                        .context("deny remote session")?;
                    println!("denied");
                }
                RemoteCommand::End { session_id } => {
                    proxy
                        .end_remote_session(&session_id)
                        .await
                        .context("end remote session")?;
                    println!("ended");
                }
                RemoteCommand::Status => {
                    let session = proxy
                        .get_remote_session()
                        .await
                        .context("get remote session")?;
                    if session.is_empty() {
                        println!("idle");
                    } else {
                        for key in [
                            ipc::REMOTE_SESSION_ID,
                            ipc::REMOTE_USER_ID,
                            ipc::REMOTE_FINGERPRINT,
                            ipc::REMOTE_ROLE,
                            ipc::REMOTE_STATE,
                            ipc::REMOTE_INDICATOR,
                            ipc::REMOTE_EXPIRES_AT,
                        ] {
                            println!("{key}: {}", message_value(&session, key));
                        }
                    }
                }
            }
        }
        Command::Policy { command } => {
            let proxy = connect().await?;
            match command {
                PolicyCommand::Set { user_id, policy } => {
                    proxy
                        .set_user_policy(&user_id, &policy)
                        .await
                        .context("set user policy")?;
                    println!("{user_id} {policy}");
                }
                PolicyCommand::Dnd { state } => {
                    let enabled = matches!(state, DndState::On);
                    proxy.set_dnd(enabled).await.context("set dnd")?;
                    println!("dnd {}", if enabled { "on" } else { "off" });
                }
            }
        }
    }
    Ok(())
}

async fn derive_conversation(proxy: &DaemonProxy<'_>, user_id: &str) -> Result<String> {
    let identity = proxy.get_identity().await.context("get daemon identity")?;
    let local = identity_value(&identity, ipc::IDENTITY_NODE_ID);
    let local = local
        .parse::<NodeId>()
        .context("daemon returned an invalid node id")?;
    let user = user_id
        .parse::<NodeId>()
        .context("user id is not a valid node id")?;
    Ok(conversation_id(local, user))
}

async fn run_users(command: UsersCommand) -> Result<()> {
    let proxy = connect().await?;
    match command {
        UsersCommand::List => {
            let users = proxy.list_users().await.context("list users")?;
            if users.is_empty() {
                println!("no users");
            } else {
                for user in users {
                    println!(
                        "{} {} {} {}",
                        user_value(&user, ipc::USER_ID),
                        user_value(&user, ipc::USER_FINGERPRINT),
                        user_value(&user, ipc::USER_POLICY),
                        user_value(&user, ipc::USER_DISPLAY_NAME)
                    );
                }
            }
        }
        UsersCommand::Forget { user_id } => {
            proxy.forget_user(&user_id).await.context("forget user")?;
            println!("user forgotten and blocked");
        }
        UsersCommand::Blocked => {
            let users = proxy
                .list_blocked_users()
                .await
                .context("list blocked users")?;
            if users.is_empty() {
                println!("no blocked users");
            } else {
                for user in users {
                    println!(
                        "{} {} {} {}",
                        user_value(&user, ipc::USER_FINGERPRINT),
                        user_value(&user, ipc::USER_ID),
                        user_value(&user, ipc::USER_DISPLAY_NAME),
                        user_value(&user, ipc::BLOCKED_AT),
                    );
                }
            }
        }
        UsersCommand::Unblock { fingerprint } => {
            proxy
                .unblock_user(&fingerprint)
                .await
                .context("unblock user")?;
            println!("user unblocked");
        }
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

fn attachment_label(message: &std::collections::HashMap<String, zbus::OwnedVariant>) -> String {
    let name = message_value(message, ipc::MESSAGE_ATTACHMENT_NAME);
    let hash = message_value(message, ipc::MESSAGE_ATTACHMENT_HASH);
    if name.is_empty() {
        message_value(message, ipc::MESSAGE_BODY)
    } else {
        format!("[{name} {hash}]")
    }
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

fn user_value(user: &std::collections::HashMap<String, zbus::OwnedVariant>, key: &str) -> String {
    user.get(key)
        .map(variant_text)
        .unwrap_or_else(|| "unknown".to_owned())
}

fn message_value(
    message: &std::collections::HashMap<String, zbus::OwnedVariant>,
    key: &str,
) -> String {
    message
        .get(key)
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
