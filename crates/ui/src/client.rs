use std::collections::{BTreeMap, HashMap};

use futures_util::StreamExt;
use ticonnect_core::ipc;
use ticonnect_core::ipc::client::DaemonProxy;
use tokio::sync::{mpsc, oneshot};
use zbus::OwnedVariant;

use crate::error::{Error, Result};
use crate::model::{Candidate, Message, MessageState, Peer, PeerPolicy, SettingsForm, short_id};

type Reply<T> = oneshot::Sender<Result<T>>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Identity {
    pub node_id: String,
    pub fingerprint: String,
    pub display_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Status {
    pub transport_state: String,
    pub active_sessions: u32,
    pub outbox_depth: u32,
    pub peer_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PairingStart {
    pub pin: String,
    pub qr: String,
    pub expires_at: u64,
    pub node_id: String,
    pub port: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DaemonEvent {
    Connected,
    Disconnected(String),
    Peers(Vec<Peer>),
    Candidates(Vec<Candidate>),
    CandidateLost(String),
    MessageReceived(Message),
    MessageStateChanged {
        message_id: String,
        state: MessageState,
    },
    PeerConnected(Peer),
    PeerDisconnected(Peer),
    PairingStarted(PairingStart),
    PairingCompleted {
        peer_id: String,
        fingerprint: String,
    },
    PairingFailed(String),
    PolicyChanged {
        peer_id: String,
        policy: PeerPolicy,
    },
    SettingsChanged(String),
    SyncCompleted {
        peer_id: String,
        count: u32,
    },
}

enum Command {
    GetIdentity(Reply<Identity>),
    GetStatus(Reply<Status>),
    ListPeers(Reply<Vec<Peer>>),
    ListCandidates(Reply<Vec<Candidate>>),
    ListSettings(Reply<SettingsForm>),
    StartPairing(Reply<PairingStart>),
    PairWithPin {
        node_id: String,
        pin: String,
        reply: Reply<()>,
    },
    PairWithQr {
        payload: String,
        reply: Reply<()>,
    },
    CancelPairing(Reply<()>),
    ListMessages {
        conversation_id: String,
        limit: u32,
        offset: u32,
        reply: Reply<Vec<Message>>,
    },
    CountMessages {
        conversation_id: String,
        reply: Reply<u32>,
    },
    SendMessage {
        peer_id: String,
        body: String,
        reply: Reply<String>,
    },
    MarkRead {
        conversation_id: String,
        message_ids: Vec<String>,
        reply: Reply<()>,
    },
    SetPeerPolicy {
        peer_id: String,
        policy: PeerPolicy,
        reply: Reply<()>,
    },
    SetDnd {
        enabled: bool,
        reply: Reply<()>,
    },
}

#[derive(Clone)]
pub struct LinkHandle {
    commands: mpsc::UnboundedSender<Command>,
}

impl LinkHandle {
    pub async fn identity(&self) -> Result<Identity> {
        self.request(Command::GetIdentity).await
    }

    pub async fn status(&self) -> Result<Status> {
        self.request(Command::GetStatus).await
    }

    pub async fn list_peers(&self) -> Result<Vec<Peer>> {
        self.request(Command::ListPeers).await
    }

    pub async fn list_candidates(&self) -> Result<Vec<Candidate>> {
        self.request(Command::ListCandidates).await
    }

    pub async fn list_settings(&self) -> Result<SettingsForm> {
        self.request(Command::ListSettings).await
    }

    pub async fn start_pairing(&self) -> Result<PairingStart> {
        self.request(Command::StartPairing).await
    }

    pub async fn pair_with_pin(&self, node_id: String, pin: String) -> Result<()> {
        self.request(|reply| Command::PairWithPin {
            node_id,
            pin,
            reply,
        })
        .await
    }

    pub async fn pair_with_qr(&self, payload: String) -> Result<()> {
        self.request(|reply| Command::PairWithQr { payload, reply })
            .await
    }

    pub async fn cancel_pairing(&self) -> Result<()> {
        self.request(Command::CancelPairing).await
    }

    pub async fn list_messages(
        &self,
        conversation_id: String,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<Message>> {
        self.request(|reply| Command::ListMessages {
            conversation_id,
            limit,
            offset,
            reply,
        })
        .await
    }

    pub async fn count_messages(&self, conversation_id: String) -> Result<u32> {
        self.request(|reply| Command::CountMessages {
            conversation_id,
            reply,
        })
        .await
    }

    pub async fn send_message(&self, peer_id: String, body: String) -> Result<String> {
        self.request(|reply| Command::SendMessage {
            peer_id,
            body,
            reply,
        })
        .await
    }

    pub async fn mark_read(&self, conversation_id: String, message_ids: Vec<String>) -> Result<()> {
        self.request(|reply| Command::MarkRead {
            conversation_id,
            message_ids,
            reply,
        })
        .await
    }

    pub async fn set_peer_policy(&self, peer_id: String, policy: PeerPolicy) -> Result<()> {
        self.request(|reply| Command::SetPeerPolicy {
            peer_id,
            policy,
            reply,
        })
        .await
    }

    pub async fn set_dnd(&self, enabled: bool) -> Result<()> {
        self.request(|reply| Command::SetDnd { enabled, reply })
            .await
    }

    async fn request<T>(&self, make: impl FnOnce(Reply<T>) -> Command) -> Result<T> {
        let (reply, receiver) = oneshot::channel();
        self.commands
            .send(make(reply))
            .map_err(|_| Error::DaemonUnavailable("daemon link stopped".to_owned()))?;
        receiver
            .await
            .map_err(|_| Error::DaemonUnavailable("daemon link stopped".to_owned()))?
    }
}

pub fn spawn_link(events: mpsc::UnboundedSender<DaemonEvent>) -> LinkHandle {
    let (commands, receiver) = mpsc::unbounded_channel();
    relm4::spawn(run_link(receiver, events));
    LinkHandle { commands }
}

async fn run_link(
    mut commands: mpsc::UnboundedReceiver<Command>,
    events: mpsc::UnboundedSender<DaemonEvent>,
) {
    let mut backoff = crate::model::Backoff::default();
    loop {
        match connect().await {
            Ok((connection, proxy)) => {
                backoff.reset();
                let _ = events.send(DaemonEvent::Connected);
                let (lost_sender, mut lost_receiver) = mpsc::unbounded_channel();
                spawn_listener(connection, events.clone(), lost_sender);

                loop {
                    tokio::select! {
                        command = commands.recv() => {
                            let Some(command) = command else {
                                return;
                            };
                            handle_command(&proxy, command).await;
                        }
                        reason = lost_receiver.recv() => {
                            let reason = reason.unwrap_or_else(|| "daemon connection closed".to_owned());
                            let _ = events.send(DaemonEvent::Disconnected(reason));
                            break;
                        }
                    }
                }
            }
            Err(error) => {
                let _ = events.send(DaemonEvent::Disconnected(error.to_string()));
                tokio::time::sleep(std::time::Duration::from_millis(backoff.next_delay())).await;
            }
        }
    }
}

async fn connect() -> Result<(zbus::Connection, DaemonProxy<'static>)> {
    let connection = zbus::Connection::session().await.map_err(bus_error)?;
    let proxy = DaemonProxy::new(&connection).await.map_err(bus_error)?;
    proxy.ping().await.map_err(bus_error)?;
    Ok((connection, proxy))
}

fn spawn_listener(
    connection: zbus::Connection,
    events: mpsc::UnboundedSender<DaemonEvent>,
    lost: mpsc::UnboundedSender<String>,
) {
    relm4::spawn(async move {
        if let Err(error) = listen(connection, events, lost.clone()).await {
            let _ = lost.send(error.to_string());
        }
    });
}

async fn listen(
    connection: zbus::Connection,
    events: mpsc::UnboundedSender<DaemonEvent>,
    lost: mpsc::UnboundedSender<String>,
) -> Result<()> {
    let proxy = DaemonProxy::new(&connection).await.map_err(bus_error)?;
    let mut messages = proxy.receive_message_received().await.map_err(bus_error)?;
    let mut states = proxy
        .receive_message_state_changed()
        .await
        .map_err(bus_error)?;
    let mut connected = proxy.receive_peer_connected().await.map_err(bus_error)?;
    let mut disconnected = proxy.receive_peer_disconnected().await.map_err(bus_error)?;
    let mut pairing_started = proxy.receive_pairing_started().await.map_err(bus_error)?;
    let mut pairing_completed = proxy.receive_pairing_completed().await.map_err(bus_error)?;
    let mut pairing_failed = proxy.receive_pairing_failed().await.map_err(bus_error)?;
    let mut policy_changed = proxy.receive_policy_changed().await.map_err(bus_error)?;
    let mut settings_changed = proxy.receive_settings_changed().await.map_err(bus_error)?;
    let mut sync_completed = proxy.receive_sync_completed().await.map_err(bus_error)?;
    let mut candidate_found = proxy.receive_candidate_found().await.map_err(bus_error)?;
    let mut candidate_lost = proxy.receive_candidate_lost().await.map_err(bus_error)?;

    let dbus = zbus::fdo::DBusProxy::new(&connection)
        .await
        .map_err(bus_error)?;
    let mut owner_changed = dbus.receive_name_owner_changed().await.map_err(bus_error)?;

    loop {
        tokio::select! {
            signal = messages.next() => {
                let Some(signal) = signal else { break };
                let Ok(args) = signal.args() else { continue };
                if let Ok(message) = parse_message(&args.message, None) {
                    let _ = events.send(DaemonEvent::MessageReceived(message));
                }
            }
            signal = states.next() => {
                let Some(signal) = signal else { break };
                let Ok(args) = signal.args() else { continue };
                let _ = events.send(DaemonEvent::MessageStateChanged {
                    message_id: args.message_id.to_owned(),
                    state: MessageState::parse(args.state),
                });
            }
            signal = connected.next() => {
                let Some(signal) = signal else { break };
                let Ok(args) = signal.args() else { continue };
                if let Ok(peer) = parse_peer(&args.peer) {
                    let _ = events.send(DaemonEvent::PeerConnected(peer));
                }
            }
            signal = disconnected.next() => {
                let Some(signal) = signal else { break };
                let Ok(args) = signal.args() else { continue };
                if let Ok(peer) = parse_peer(&args.peer) {
                    let _ = events.send(DaemonEvent::PeerDisconnected(peer));
                }
            }
            signal = pairing_started.next() => {
                let Some(signal) = signal else { break };
                let Ok(args) = signal.args() else { continue };
                if let Ok(start) = parse_pairing(&args.pairing) {
                    let _ = events.send(DaemonEvent::PairingStarted(start));
                }
            }
            signal = pairing_completed.next() => {
                let Some(signal) = signal else { break };
                let Ok(args) = signal.args() else { continue };
                let _ = events.send(DaemonEvent::PairingCompleted {
                    peer_id: args.peer_id.to_owned(),
                    fingerprint: args.fingerprint.to_owned(),
                });
            }
            signal = pairing_failed.next() => {
                let Some(signal) = signal else { break };
                let Ok(args) = signal.args() else { continue };
                let _ = events.send(DaemonEvent::PairingFailed(args.reason.to_owned()));
            }
            signal = policy_changed.next() => {
                let Some(signal) = signal else { break };
                let Ok(args) = signal.args() else { continue };
                let _ = events.send(DaemonEvent::PolicyChanged {
                    peer_id: args.peer_id.to_owned(),
                    policy: PeerPolicy::parse(args.policy),
                });
            }
            signal = settings_changed.next() => {
                let Some(signal) = signal else { break };
                let Ok(args) = signal.args() else { continue };
                let _ = events.send(DaemonEvent::SettingsChanged(args.key.to_owned()));
            }
            signal = sync_completed.next() => {
                let Some(signal) = signal else { break };
                let Ok(args) = signal.args() else { continue };
                let _ = events.send(DaemonEvent::SyncCompleted {
                    peer_id: args.peer_id.to_owned(),
                    count: args.count,
                });
            }
            signal = candidate_found.next() => {
                let Some(signal) = signal else { break };
                let Ok(args) = signal.args() else { continue };
                if let Ok(candidate) = parse_candidate(&args.candidate) {
                    let _ = events.send(DaemonEvent::Candidates(vec![candidate]));
                }
            }
            signal = candidate_lost.next() => {
                let Some(signal) = signal else { break };
                let Ok(args) = signal.args() else { continue };
                let _ = events.send(DaemonEvent::CandidateLost(args.fingerprint.to_owned()));
                tracing::debug!(fingerprint = %args.fingerprint, "candidate lost");
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

async fn handle_command(proxy: &DaemonProxy<'_>, command: Command) {
    match command {
        Command::GetIdentity(reply) => {
            let result = match proxy.get_identity().await {
                Ok(values) => parse_identity(&values),
                Err(error) => Err(bus_error(error)),
            };
            let _ = reply.send(result);
        }
        Command::GetStatus(reply) => {
            let result = match proxy.get_status().await {
                Ok(values) => parse_status(&values),
                Err(error) => Err(bus_error(error)),
            };
            let _ = reply.send(result);
        }
        Command::ListPeers(reply) => {
            let result = match proxy.list_peers().await {
                Ok(values) => parse_peers(&values),
                Err(error) => Err(bus_error(error)),
            };
            let _ = reply.send(result);
        }
        Command::ListCandidates(reply) => {
            let result = match proxy.list_candidates().await {
                Ok(values) => values
                    .iter()
                    .map(parse_candidate)
                    .collect::<Result<Vec<_>>>(),
                Err(error) => Err(bus_error(error)),
            };
            let _ = reply.send(result);
        }
        Command::ListSettings(reply) => {
            let result = match proxy.list_settings().await {
                Ok(values) => Ok(parse_settings(&values)),
                Err(error) => Err(bus_error(error)),
            };
            let _ = reply.send(result);
        }
        Command::StartPairing(reply) => {
            let result = match proxy.start_pairing().await {
                Ok(values) => parse_pairing(&values),
                Err(error) => Err(bus_error(error)),
            };
            let _ = reply.send(result);
        }
        Command::PairWithPin {
            node_id,
            pin,
            reply,
        } => {
            let result = proxy
                .pair_with_pin(&node_id, &pin)
                .await
                .map(|_| ())
                .map_err(bus_error);
            let _ = reply.send(result);
        }
        Command::PairWithQr { payload, reply } => {
            let result = proxy
                .pair_with_qr(&payload)
                .await
                .map(|_| ())
                .map_err(bus_error);
            let _ = reply.send(result);
        }
        Command::CancelPairing(reply) => {
            let result = proxy.cancel_pairing().await.map_err(bus_error);
            let _ = reply.send(result);
        }
        Command::ListMessages {
            conversation_id,
            limit,
            offset,
            reply,
        } => {
            let result = match proxy.list_messages(&conversation_id, limit, offset).await {
                Ok(values) => values
                    .iter()
                    .map(|message| parse_message(message, None))
                    .collect::<Result<Vec<_>>>(),
                Err(error) => Err(bus_error(error)),
            };
            let _ = reply.send(result);
        }
        Command::CountMessages {
            conversation_id,
            reply,
        } => {
            let result = proxy
                .count_messages(&conversation_id)
                .await
                .map_err(bus_error);
            let _ = reply.send(result);
        }
        Command::SendMessage {
            peer_id,
            body,
            reply,
        } => {
            let result = proxy.send_message(&peer_id, &body).await.map_err(bus_error);
            let _ = reply.send(result);
        }
        Command::MarkRead {
            conversation_id,
            message_ids,
            reply,
        } => {
            let ids: Vec<&str> = message_ids.iter().map(String::as_str).collect();
            let result = proxy
                .mark_read(&conversation_id, &ids)
                .await
                .map_err(bus_error);
            let _ = reply.send(result);
        }
        Command::SetPeerPolicy {
            peer_id,
            policy,
            reply,
        } => {
            let result = proxy
                .set_peer_policy(&peer_id, policy.as_str())
                .await
                .map_err(bus_error);
            let _ = reply.send(result);
        }
        Command::SetDnd { enabled, reply } => {
            let result = proxy.set_dnd(enabled).await.map_err(bus_error);
            let _ = reply.send(result);
        }
    }
}

fn bus_error(error: zbus::Error) -> Error {
    Error::DaemonUnavailable(error.to_string())
}

fn parse_identity(values: &HashMap<String, OwnedVariant>) -> Result<Identity> {
    Ok(Identity {
        node_id: required_string(values, ipc::IDENTITY_NODE_ID)?,
        fingerprint: required_string(values, ipc::IDENTITY_FINGERPRINT)?,
        display_name: optional_string(values, ipc::IDENTITY_DISPLAY_NAME),
    })
}

fn parse_status(values: &HashMap<String, OwnedVariant>) -> Result<Status> {
    Ok(Status {
        transport_state: optional_string(values, ipc::STATUS_TRANSPORT_STATE),
        active_sessions: optional_u64(values, ipc::STATUS_ACTIVE_SESSIONS) as u32,
        outbox_depth: optional_u64(values, ipc::STATUS_OUTBOX_DEPTH) as u32,
        peer_count: optional_u64(values, ipc::STATUS_PEER_COUNT) as u32,
    })
}

fn parse_peers(values: &[HashMap<String, OwnedVariant>]) -> Result<Vec<Peer>> {
    values.iter().map(parse_peer).collect()
}

fn parse_peer(values: &HashMap<String, OwnedVariant>) -> Result<Peer> {
    Ok(Peer {
        id: required_string(values, ipc::PEER_ID)?,
        fingerprint: required_string(values, ipc::PEER_FINGERPRINT)?,
        display_name: optional_string(values, ipc::PEER_DISPLAY_NAME),
        policy: PeerPolicy::parse(&optional_string(values, ipc::PEER_POLICY)),
        trusted_at: optional_u64(values, ipc::PEER_TRUSTED_AT),
        last_seen: optional_u64(values, ipc::PEER_LAST_SEEN),
        connected: optional_bool(values, ipc::PEER_CONNECTED),
    })
}

fn parse_candidate(values: &HashMap<String, OwnedVariant>) -> Result<Candidate> {
    Ok(Candidate {
        node_id: required_string(values, ipc::CANDIDATE_NODE_ID)?,
        fingerprint: required_string(values, ipc::CANDIDATE_FINGERPRINT)?,
        display_name: optional_string(values, ipc::CANDIDATE_DISPLAY_NAME),
        addresses: optional_string(values, ipc::CANDIDATE_ADDRESSES),
        source: optional_string(values, ipc::CANDIDATE_SOURCE),
    })
}

fn parse_message(
    values: &HashMap<String, OwnedVariant>,
    local_node_id: Option<&str>,
) -> Result<Message> {
    let sender_id = required_string(values, ipc::MESSAGE_SENDER_ID)?;
    let local_sender = local_node_id.is_some_and(|local| local == sender_id);
    Ok(Message {
        id: required_string(values, ipc::MESSAGE_ID)?,
        conversation_id: required_string(values, ipc::MESSAGE_CONVERSATION_ID)?,
        peer_id: optional_string(values, ipc::MESSAGE_PEER_ID),
        sender_id,
        local_sender,
        seq: optional_u64(values, ipc::MESSAGE_SEQ),
        sent_at: optional_u64(values, ipc::MESSAGE_SENT_AT),
        received_at: optional_u64(values, ipc::MESSAGE_RECEIVED_AT),
        body: optional_string(values, ipc::MESSAGE_BODY),
        state: MessageState::parse(&optional_string(values, ipc::MESSAGE_STATE)),
    })
}

fn parse_pairing(values: &HashMap<String, OwnedVariant>) -> Result<PairingStart> {
    Ok(PairingStart {
        pin: required_string(values, ipc::PAIRING_PIN)?,
        qr: required_string(values, ipc::PAIRING_QR)?,
        expires_at: optional_u64(values, ipc::PAIRING_EXPIRES_AT),
        node_id: optional_string(values, ipc::PAIRING_NODE_ID),
        port: optional_u64(values, ipc::PAIRING_PORT) as u32,
    })
}

fn parse_settings(values: &HashMap<String, OwnedVariant>) -> SettingsForm {
    let mut settings = BTreeMap::new();
    for (key, value) in values {
        settings.insert(key.clone(), display_value(value));
    }
    SettingsForm::new(settings)
}

pub fn parse_activation_view(value: &str) -> Result<&'static str> {
    crate::model::pairing_view(value)
}

fn required_string(values: &HashMap<String, OwnedVariant>, key: &str) -> Result<String> {
    values
        .get(key)
        .map(display_value)
        .ok_or_else(|| Error::InvalidResponse(format!("missing field: {key}")))
}

fn optional_string(values: &HashMap<String, OwnedVariant>, key: &str) -> String {
    values.get(key).map(display_value).unwrap_or_default()
}

fn optional_u64(values: &HashMap<String, OwnedVariant>, key: &str) -> u64 {
    values
        .get(key)
        .and_then(|value| u64::try_from(value).ok())
        .unwrap_or_default()
}

fn optional_bool(values: &HashMap<String, OwnedVariant>, key: &str) -> bool {
    values
        .get(key)
        .and_then(|value| bool::try_from(value).ok())
        .unwrap_or_default()
}

fn display_value(value: &OwnedVariant) -> String {
    if let Ok(value) = u64::try_from(value) {
        value.to_string()
    } else if let Ok(value) = u32::try_from(value) {
        value.to_string()
    } else if let Ok(value) = bool::try_from(value) {
        value.to_string()
    } else if let Ok(value) = <&str>::try_from(value) {
        value.to_owned()
    } else {
        String::new()
    }
}

pub fn peer_error_label(peer: &Peer) -> String {
    format!("{} ({})", peer.display_label(), short_id(&peer.id))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn activation_view_rejects_unknown() {
        assert_eq!(parse_activation_view("chat").expect("chat"), "chat");
        assert!(parse_activation_view("nope").is_err());
    }

    #[test]
    fn invalid_required_field_is_reported() {
        let values = HashMap::new();
        assert!(required_string(&values, ipc::PEER_ID).is_err());
    }

    #[test]
    fn daemon_errors_map_to_ui_errors() {
        let error = bus_error(zbus::Error::NameTaken);
        assert!(matches!(error, Error::DaemonUnavailable(_)));
    }

    #[test]
    fn renders_setting_values() {
        let mut values = HashMap::new();
        values.insert("x".to_owned(), OwnedVariant::from(true));
        values.insert("y".to_owned(), OwnedVariant::from(7_u32));
        assert_eq!(display_value(&values["x"]), "true");
        assert_eq!(display_value(&values["y"]), "7");
    }
}
