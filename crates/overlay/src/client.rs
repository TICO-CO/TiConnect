use std::time::Duration;

use futures_util::StreamExt;
use ticonnect_core::ipc;
use ticonnect_core::ipc::client::DaemonProxy;
use ticonnect_core::ipc::events::{self, Call, Message, RemoteSession, User, WatchParty};
use ticonnect_settings::Settings;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

const DEFAULT_TIMEOUT_SECS: u64 = 5;
const RECONNECT_MIN_MS: u64 = 1_000;
const RECONNECT_MAX_MS: u64 = 10_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Activate(String),
    AcceptCall(String),
    DeclineCall(String),
    HangupCall(String),
    AcceptRemoteSession(String),
    DenyRemoteSession(String),
    EndRemoteSession(String),
    AcceptWatchParty(String),
    DeclineWatchParty(String),
    PublishWatchParty {
        party_id: String,
        position_ms: u64,
        playing: bool,
    },
    EndWatchParty(String),
}

#[derive(Debug, Clone)]
pub enum Event {
    Settings { enabled: bool, timeout_secs: u64 },
    Users(Vec<User>),
    User(User),
    Message(Message),
    IncomingCall(Call),
    CallState(Option<Call>),
    RemoteSessionRequested(RemoteSession),
    RemoteState(Option<RemoteSession>),
    CallEnded(String),
    RemoteSessionEnded(String),
    WatchPartyRequested(WatchParty),
    WatchPartyChanged(WatchParty),
    WatchPartyEnded(String),
    Disconnected(String),
}

pub fn spawn(events: UnboundedSender<Event>, commands: UnboundedReceiver<Command>) {
    std::thread::spawn(move || {
        let runtime = match tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
        {
            Ok(runtime) => runtime,
            Err(error) => {
                tracing::error!(%error, "failed to start overlay runtime");
                return;
            }
        };
        runtime.block_on(run(events, commands));
    });
}

async fn run(events: UnboundedSender<Event>, mut commands: UnboundedReceiver<Command>) {
    let mut backoff = RECONNECT_MIN_MS;
    loop {
        match connect().await {
            Ok((connection, proxy)) => {
                backoff = RECONNECT_MIN_MS;
                let settings = load_settings().await;
                let _ = events.send(settings);
                if let Ok(values) = proxy.list_users().await
                    && let Ok(users) = events::parse_users(&values)
                {
                    let _ = events.send(Event::Users(users));
                }

                let (lost_tx, mut lost_rx) = tokio::sync::mpsc::unbounded_channel();
                spawn_listener(connection.clone(), events.clone(), lost_tx);

                loop {
                    tokio::select! {
                        command = commands.recv() => {
                            let Some(command) = command else {
                                return;
                            };
                            handle_command(&connection, command).await;
                        }
                        reason = lost_rx.recv() => {
                            let reason = reason.unwrap_or_else(|| "daemon connection closed".to_owned());
                            let _ = events.send(Event::Disconnected(reason));
                            break;
                        }
                    }
                }
            }
            Err(error) => {
                let _ = events.send(Event::Disconnected(error.to_string()));
                tokio::time::sleep(Duration::from_millis(backoff)).await;
                backoff = (backoff * 2).min(RECONNECT_MAX_MS);
            }
        }
    }
}

async fn connect() -> zbus::Result<(zbus::Connection, DaemonProxy<'static>)> {
    let connection = zbus::Connection::session().await?;
    let proxy = DaemonProxy::new(&connection).await?;
    proxy.ping().await?;
    Ok((connection, proxy))
}

async fn load_settings() -> Event {
    let loaded = tokio::task::spawn_blocking(Settings::load).await;
    match loaded {
        Ok(Ok(settings)) => Event::Settings {
            enabled: settings.overlay_enabled(),
            timeout_secs: u64::from(settings.overlay_timeout_secs()),
        },
        Ok(Err(error)) => {
            tracing::warn!(%error, "failed to load settings; using overlay defaults");
            default_settings_event()
        }
        Err(error) => {
            tracing::warn!(%error, "settings load task failed; using overlay defaults");
            default_settings_event()
        }
    }
}

fn default_settings_event() -> Event {
    Event::Settings {
        enabled: true,
        timeout_secs: DEFAULT_TIMEOUT_SECS,
    }
}

fn spawn_listener(
    connection: zbus::Connection,
    events: UnboundedSender<Event>,
    lost: UnboundedSender<String>,
) {
    tokio::spawn(async move {
        if let Err(error) = listen(connection, events, lost.clone()).await {
            let _ = lost.send(error.to_string());
        }
    });
}

async fn listen(
    connection: zbus::Connection,
    events: UnboundedSender<Event>,
    lost: UnboundedSender<String>,
) -> zbus::Result<()> {
    let proxy = DaemonProxy::new(&connection).await?;
    let mut messages = proxy.receive_message_received().await?;
    let mut connected = proxy.receive_user_connected().await?;
    let mut disconnected = proxy.receive_user_disconnected().await?;
    let mut settings_changed = proxy.receive_settings_changed().await?;
    let mut incoming_call = proxy.receive_incoming_call().await?;
    let mut call_state_changed = proxy.receive_call_state_changed().await?;
    let mut remote_requested = proxy.receive_remote_session_requested().await?;
    let mut remote_started = proxy.receive_remote_session_started().await?;
    let mut remote_indicator = proxy.receive_remote_indicator_changed().await?;
    let mut call_ended = proxy.receive_call_ended().await?;
    let mut remote_ended = proxy.receive_remote_session_ended().await?;
    let mut watch_party_requested = proxy.receive_watch_party_requested().await?;
    let mut watch_party_changed = proxy.receive_watch_party_changed().await?;
    let mut watch_party_ended = proxy.receive_watch_party_ended().await?;

    let dbus = zbus::fdo::DBusProxy::new(&connection).await?;
    let mut owner_changed = dbus.receive_name_owner_changed().await?;

    loop {
        tokio::select! {
            signal = messages.next() => {
                let Some(signal) = signal else { break };
                let Ok(args) = signal.args() else { continue };
                if let Ok(message) = events::parse_message(&args.message, None) {
                    let _ = events.send(Event::Message(message));
                }
            }
            signal = connected.next() => {
                let Some(signal) = signal else { break };
                let Ok(args) = signal.args() else { continue };
                if let Ok(user) = events::parse_user(&args.user) {
                    let _ = events.send(Event::User(user));
                }
            }
            signal = disconnected.next() => {
                let Some(signal) = signal else { break };
                let Ok(args) = signal.args() else { continue };
                if let Ok(user) = events::parse_user(&args.user) {
                    let _ = events.send(Event::User(user));
                }
            }
            signal = incoming_call.next() => {
                let Some(signal) = signal else { break };
                let Ok(args) = signal.args() else { continue };
                if let Ok(call) = events::parse_call(&args.call) {
                    let _ = events.send(Event::IncomingCall(call));
                }
            }
            signal = call_state_changed.next() => {
                let Some(signal) = signal else { break };
                let Ok(_args) = signal.args() else { continue };
                if let Ok(proxy) = DaemonProxy::new(&connection).await
                    && let Ok(values) = proxy.get_call_state().await
                {
                    let call = if values.is_empty() {
                        None
                    } else {
                        events::parse_call(&values).ok()
                    };
                    let _ = events.send(Event::CallState(call));
                }
            }
            signal = remote_requested.next() => {
                let Some(signal) = signal else { break };
                let Ok(args) = signal.args() else { continue };
                if let Ok(session) = events::parse_remote_session(&args.session) {
                    let _ = events.send(Event::RemoteSessionRequested(session));
                }
            }
            signal = remote_started.next() => {
                let Some(signal) = signal else { break };
                let Ok(_args) = signal.args() else { continue };
                if let Ok(proxy) = DaemonProxy::new(&connection).await
                    && let Ok(values) = proxy.get_remote_session().await
                {
                    let session = if values.is_empty() {
                        None
                    } else {
                        events::parse_remote_session(&values).ok()
                    };
                    let _ = events.send(Event::RemoteState(session));
                }
            }
            signal = remote_indicator.next() => {
                let Some(signal) = signal else { break };
                let Ok(_args) = signal.args() else { continue };
                if let Ok(proxy) = DaemonProxy::new(&connection).await
                    && let Ok(values) = proxy.get_remote_session().await
                {
                    let session = if values.is_empty() {
                        None
                    } else {
                        events::parse_remote_session(&values).ok()
                    };
                    let _ = events.send(Event::RemoteState(session));
                }
            }
            signal = watch_party_requested.next() => {
                let Some(signal) = signal else { break };
                let Ok(args) = signal.args() else { continue };
                if let Ok(party) = events::parse_watch_party(&args.party) {
                    let _ = events.send(Event::WatchPartyRequested(party));
                }
            }
            signal = watch_party_changed.next() => {
                let Some(signal) = signal else { break };
                let Ok(args) = signal.args() else { continue };
                if let Ok(party) = events::parse_watch_party(&args.party) {
                    let _ = events.send(Event::WatchPartyChanged(party));
                }
            }
            signal = watch_party_ended.next() => {
                let Some(signal) = signal else { break };
                let Ok(args) = signal.args() else { continue };
                let _ = events.send(Event::WatchPartyEnded(args.party_id.to_owned()));
            }
            signal = call_ended.next() => {
                let Some(signal) = signal else { break };
                let Ok(args) = signal.args() else { continue };
                let _ = events.send(Event::CallEnded(args.call_id.to_owned()));
            }
            signal = remote_ended.next() => {
                let Some(signal) = signal else { break };
                let Ok(args) = signal.args() else { continue };
                let _ = events.send(Event::RemoteSessionEnded(args.session_id.to_owned()));
            }
            signal = settings_changed.next() => {
                let Some(signal) = signal else { break };
                let Ok(_args) = signal.args() else { continue };
                let _ = events.send(load_settings().await);
            }
            signal = owner_changed.next() => {
                let Some(signal) = signal else { break };
                let Ok(args) = signal.args() else { continue };
                if args.name == ipc::DAEMON_BUS_NAME {
                    let _ = lost.send("daemon restarting".to_owned());
                    return Ok(());
                }
            }
            else => break,
        }
    }

    let _ = lost.send("daemon signal stream ended".to_owned());
    Ok(())
}

async fn handle_command(connection: &zbus::Connection, command: Command) {
    match command {
        Command::Activate(view) => {
            if let Err(error) = activate(connection, &view).await {
                tracing::warn!(%error, "failed to activate UI");
            }
        }
        Command::AcceptCall(call_id) => {
            let proxy = DaemonProxy::new(connection).await;
            if let Ok(proxy) = proxy
                && let Err(error) = proxy.accept_call(&call_id).await
            {
                tracing::warn!(%error, "failed to accept call");
            }
        }
        Command::DeclineCall(call_id) => {
            let proxy = DaemonProxy::new(connection).await;
            if let Ok(proxy) = proxy
                && let Err(error) = proxy.decline_call(&call_id).await
            {
                tracing::warn!(%error, "failed to decline call");
            }
        }
        Command::HangupCall(call_id) => {
            let proxy = DaemonProxy::new(connection).await;
            if let Ok(proxy) = proxy
                && let Err(error) = proxy.hangup_call(&call_id).await
            {
                tracing::warn!(%error, "failed to hang up call");
            }
        }
        Command::AcceptRemoteSession(session_id) => {
            let proxy = DaemonProxy::new(connection).await;
            if let Ok(proxy) = proxy
                && let Err(error) = proxy.accept_remote_session(&session_id).await
            {
                tracing::warn!(%error, "failed to accept remote session");
            }
        }
        Command::DenyRemoteSession(session_id) => {
            let proxy = DaemonProxy::new(connection).await;
            if let Ok(proxy) = proxy
                && let Err(error) = proxy.deny_remote_session(&session_id).await
            {
                tracing::warn!(%error, "failed to deny remote session");
            }
        }
        Command::EndRemoteSession(session_id) => {
            let proxy = DaemonProxy::new(connection).await;
            if let Ok(proxy) = proxy
                && let Err(error) = proxy.end_remote_session(&session_id).await
            {
                tracing::warn!(%error, "failed to end remote session");
            }
        }
        Command::AcceptWatchParty(party_id) => {
            let proxy = DaemonProxy::new(connection).await;
            if let Ok(proxy) = proxy
                && let Err(error) = proxy.accept_watch_party(&party_id).await
            {
                tracing::warn!(%error, "failed to accept watch party");
            }
        }
        Command::DeclineWatchParty(party_id) => {
            let proxy = DaemonProxy::new(connection).await;
            if let Ok(proxy) = proxy
                && let Err(error) = proxy.decline_watch_party(&party_id).await
            {
                tracing::warn!(%error, "failed to decline watch party");
            }
        }
        Command::PublishWatchParty {
            party_id,
            position_ms,
            playing,
        } => {
            let proxy = DaemonProxy::new(connection).await;
            if let Ok(proxy) = proxy
                && let Err(error) = proxy
                    .publish_watch_party(&party_id, position_ms, playing)
                    .await
            {
                tracing::warn!(%error, "failed to publish watch party state");
            }
        }
        Command::EndWatchParty(party_id) => {
            let proxy = DaemonProxy::new(connection).await;
            if let Ok(proxy) = proxy
                && let Err(error) = proxy.end_watch_party(&party_id).await
            {
                tracing::warn!(%error, "failed to end watch party");
            }
        }
    }
}

async fn activate(connection: &zbus::Connection, view: &str) -> anyhow::Result<()> {
    match call_activate(connection, view).await {
        Ok(()) => Ok(()),
        Err(first) => {
            tracing::debug!(%first, "UI is not running; trying to start it");
            ui_command().spawn()?;
            tokio::time::sleep(Duration::from_millis(350)).await;
            call_activate(connection, view).await.map_err(Into::into)
        }
    }
}

fn ui_command() -> std::process::Command {
    let mut command = std::process::Command::new(ticonnect_runtime::main_executable());
    command.arg("ui");
    command
}

async fn call_activate(connection: &zbus::Connection, view: &str) -> zbus::Result<()> {
    let proxy = zbus::Proxy::new(
        connection,
        ipc::UI_BUS_NAME,
        ipc::UI_PATH,
        ipc::UI_INTERFACE,
    )
    .await?;
    let _: () = proxy.call("Activate", &(view,)).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ui_command_uses_current_executable_and_ui_argument() {
        let command = ui_command();
        assert_eq!(command.get_program(), ticonnect_runtime::main_executable());
        assert_eq!(
            command.get_args().collect::<Vec<_>>(),
            [std::ffi::OsStr::new("ui")]
        );
    }
}
