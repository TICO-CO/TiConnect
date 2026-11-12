use std::time::Duration;

use futures_util::StreamExt;
use ticonnect_core::ipc;
use ticonnect_core::ipc::client::DaemonProxy;
use ticonnect_core::ipc::events::{self, Message, Peer};
use ticonnect_settings::Settings;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

const DEFAULT_TIMEOUT_SECS: u64 = 5;
const RECONNECT_MIN_MS: u64 = 1_000;
const RECONNECT_MAX_MS: u64 = 10_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Activate(String),
}

#[derive(Debug, Clone)]
pub enum Event {
    Settings { enabled: bool, timeout_secs: u64 },
    Peers(Vec<Peer>),
    Peer(Peer),
    Message(Message),
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
                if let Ok(values) = proxy.list_peers().await
                    && let Ok(peers) = events::parse_peers(&values)
                {
                    let _ = events.send(Event::Peers(peers));
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
    let mut connected = proxy.receive_peer_connected().await?;
    let mut disconnected = proxy.receive_peer_disconnected().await?;
    let mut settings_changed = proxy.receive_settings_changed().await?;

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
                if let Ok(peer) = events::parse_peer(&args.peer) {
                    let _ = events.send(Event::Peer(peer));
                }
            }
            signal = disconnected.next() => {
                let Some(signal) = signal else { break };
                let Ok(args) = signal.args() else { continue };
                if let Ok(peer) = events::parse_peer(&args.peer) {
                    let _ = events.send(Event::Peer(peer));
                }
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
    }
}

async fn activate(connection: &zbus::Connection, view: &str) -> anyhow::Result<()> {
    match call_activate(connection, view).await {
        Ok(()) => Ok(()),
        Err(first) => {
            tracing::debug!(%first, "UI is not running; trying to start it");
            std::process::Command::new("ticonnect-ui").spawn()?;
            tokio::time::sleep(Duration::from_millis(350)).await;
            call_activate(connection, view).await.map_err(Into::into)
        }
    }
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
