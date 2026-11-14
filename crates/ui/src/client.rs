use std::collections::HashMap;

use futures_util::StreamExt;
use ticonnect_core::ipc;
use ticonnect_core::ipc::client::DaemonProxy;
use ticonnect_core::ipc::events::{
    self, Call, Candidate, Message, MessageState, Peer, PeerPolicy, RemoteSession,
};
pub use ticonnect_core::ipc::events::{DaemonEvent, Identity, PairingStart, Status};
use tokio::sync::{mpsc, oneshot};
use zbus::OwnedVariant;

use crate::error::{Error, Result};
use crate::model::{PeerPresentation, SettingsForm, short_id};

type Reply<T> = oneshot::Sender<Result<T>>;

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
    SendAttachment {
        peer_id: String,
        path: String,
        reply: Reply<String>,
    },
    AttachmentPath {
        hash: String,
        reply: Reply<String>,
    },
    SaveAttachment {
        hash: String,
        destination: String,
        reply: Reply<bool>,
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
    StartCall {
        peer_id: String,
        reply: Reply<String>,
    },
    AcceptCall {
        call_id: String,
        reply: Reply<()>,
    },
    DeclineCall {
        call_id: String,
        reply: Reply<()>,
    },
    HangupCall {
        call_id: String,
        reply: Reply<()>,
    },
    GetCallState(Reply<Option<Call>>),
    RequestRemoteSession {
        peer_id: String,
        reply: Reply<RemoteSession>,
    },
    AcceptRemoteSession {
        session_id: String,
        pin: String,
        reply: Reply<()>,
    },
    DenyRemoteSession {
        session_id: String,
        reply: Reply<()>,
    },
    EndRemoteSession {
        session_id: String,
        reply: Reply<()>,
    },
    GetRemoteSession(Reply<Option<RemoteSession>>),
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

    pub async fn send_attachment(&self, peer_id: String, path: String) -> Result<String> {
        self.request(|reply| Command::SendAttachment {
            peer_id,
            path,
            reply,
        })
        .await
    }

    pub async fn attachment_path(&self, hash: String) -> Result<String> {
        self.request(|reply| Command::AttachmentPath { hash, reply })
            .await
    }

    pub async fn save_attachment(&self, hash: String, destination: String) -> Result<bool> {
        self.request(|reply| Command::SaveAttachment {
            hash,
            destination,
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

    pub async fn start_call(&self, peer_id: String) -> Result<String> {
        self.request(|reply| Command::StartCall { peer_id, reply })
            .await
    }

    pub async fn accept_call(&self, call_id: String) -> Result<()> {
        self.request(|reply| Command::AcceptCall { call_id, reply })
            .await
    }

    pub async fn decline_call(&self, call_id: String) -> Result<()> {
        self.request(|reply| Command::DeclineCall { call_id, reply })
            .await
    }

    pub async fn hangup_call(&self, call_id: String) -> Result<()> {
        self.request(|reply| Command::HangupCall { call_id, reply })
            .await
    }

    pub async fn call_state(&self) -> Result<Option<Call>> {
        self.request(Command::GetCallState).await
    }

    pub async fn request_remote_session(&self, peer_id: String) -> Result<RemoteSession> {
        self.request(|reply| Command::RequestRemoteSession { peer_id, reply })
            .await
    }

    pub async fn accept_remote_session(&self, session_id: String, pin: String) -> Result<()> {
        self.request(|reply| Command::AcceptRemoteSession {
            session_id,
            pin,
            reply,
        })
        .await
    }

    pub async fn deny_remote_session(&self, session_id: String) -> Result<()> {
        self.request(|reply| Command::DenyRemoteSession { session_id, reply })
            .await
    }

    pub async fn end_remote_session(&self, session_id: String) -> Result<()> {
        self.request(|reply| Command::EndRemoteSession { session_id, reply })
            .await
    }

    pub async fn remote_session(&self) -> Result<Option<RemoteSession>> {
        self.request(Command::GetRemoteSession).await
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
    let mut transfer_progress = proxy.receive_transfer_progress().await.map_err(bus_error)?;
    let mut incoming_call = proxy.receive_incoming_call().await.map_err(bus_error)?;
    let mut call_state_changed = proxy
        .receive_call_state_changed()
        .await
        .map_err(bus_error)?;
    let mut call_ended = proxy.receive_call_ended().await.map_err(bus_error)?;
    let mut remote_requested = proxy
        .receive_remote_session_requested()
        .await
        .map_err(bus_error)?;
    let mut remote_started = proxy
        .receive_remote_session_started()
        .await
        .map_err(bus_error)?;
    let mut remote_ended = proxy
        .receive_remote_session_ended()
        .await
        .map_err(bus_error)?;
    let mut remote_indicator = proxy
        .receive_remote_indicator_changed()
        .await
        .map_err(bus_error)?;
    let mut transfer_done = proxy.receive_transfer_done().await.map_err(bus_error)?;
    let mut transfer_failed = proxy.receive_transfer_failed().await.map_err(bus_error)?;
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
                if let Ok(message) = events::parse_message(&args.message, None) {
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
                if let Ok(peer) = events::parse_peer(&args.peer) {
                    let _ = events.send(DaemonEvent::PeerConnected(peer));
                }
            }
            signal = disconnected.next() => {
                let Some(signal) = signal else { break };
                let Ok(args) = signal.args() else { continue };
                if let Ok(peer) = events::parse_peer(&args.peer) {
                    let _ = events.send(DaemonEvent::PeerDisconnected(peer));
                }
            }
            signal = pairing_started.next() => {
                let Some(signal) = signal else { break };
                let Ok(args) = signal.args() else { continue };
                if let Ok(start) = events::parse_pairing(&args.pairing) {
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
            signal = incoming_call.next() => {
                let Some(signal) = signal else { break };
                let Ok(args) = signal.args() else { continue };
                if let Ok(call) = events::parse_call(&args.call) {
                    let _ = events.send(DaemonEvent::IncomingCall(call));
                }
            }
            signal = call_state_changed.next() => {
                let Some(signal) = signal else { break };
                let Ok(args) = signal.args() else { continue };
                let _ = events.send(DaemonEvent::CallStateChanged {
                    call_id: args.call_id.to_owned(),
                    state: args.state.to_owned(),
                });
            }
            signal = call_ended.next() => {
                let Some(signal) = signal else { break };
                let Ok(args) = signal.args() else { continue };
                let _ = events.send(DaemonEvent::CallEnded {
                    call_id: args.call_id.to_owned(),
                    reason: args.reason.to_owned(),
                });
            }
            signal = remote_requested.next() => {
                let Some(signal) = signal else { break };
                let Ok(args) = signal.args() else { continue };
                if let Ok(session) = events::parse_remote_session(&args.session) {
                    let _ = events.send(DaemonEvent::RemoteSessionRequested(session));
                }
            }
            signal = remote_started.next() => {
                let Some(signal) = signal else { break };
                let Ok(args) = signal.args() else { continue };
                let _ = events.send(DaemonEvent::RemoteSessionStarted(args.session_id.to_owned()));
            }
            signal = remote_ended.next() => {
                let Some(signal) = signal else { break };
                let Ok(args) = signal.args() else { continue };
                let _ = events.send(DaemonEvent::RemoteSessionEnded {
                    session_id: args.session_id.to_owned(),
                    reason: args.reason.to_owned(),
                });
            }
            signal = remote_indicator.next() => {
                let Some(signal) = signal else { break };
                let Ok(args) = signal.args() else { continue };
                let _ = events.send(DaemonEvent::RemoteIndicatorChanged {
                    session_id: args.session_id.to_owned(),
                    visible: args.visible,
                });
            }
            signal = transfer_progress.next() => {
                let Some(signal) = signal else { break };
                let Ok(args) = signal.args() else { continue };
                let _ = events.send(DaemonEvent::TransferProgress {
                    hash: args.hash.to_owned(),
                    bytes: args.bytes,
                    total: args.total,
                });
            }
            signal = transfer_done.next() => {
                let Some(signal) = signal else { break };
                let Ok(args) = signal.args() else { continue };
                let _ = events.send(DaemonEvent::TransferDone(args.hash.to_owned()));
            }
            signal = transfer_failed.next() => {
                let Some(signal) = signal else { break };
                let Ok(args) = signal.args() else { continue };
                let _ = events.send(DaemonEvent::TransferFailed {
                    hash: args.hash.to_owned(),
                    reason: args.reason.to_owned(),
                });
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
                if let Ok(candidate) = events::parse_candidate(&args.candidate) {
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
                Ok(values) => events::parse_identity(&values).map_err(payload_error),
                Err(error) => Err(bus_error(error)),
            };
            let _ = reply.send(result);
        }
        Command::GetStatus(reply) => {
            let result = match proxy.get_status().await {
                Ok(values) => events::parse_status(&values).map_err(payload_error),
                Err(error) => Err(bus_error(error)),
            };
            let _ = reply.send(result);
        }
        Command::ListPeers(reply) => {
            let result = match proxy.list_peers().await {
                Ok(values) => events::parse_peers(&values).map_err(payload_error),
                Err(error) => Err(bus_error(error)),
            };
            let _ = reply.send(result);
        }
        Command::ListCandidates(reply) => {
            let result = match proxy.list_candidates().await {
                Ok(values) => values
                    .iter()
                    .map(|values| events::parse_candidate(values).map_err(payload_error))
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
                Ok(values) => events::parse_pairing(&values).map_err(payload_error),
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
                    .map(|message| events::parse_message(message, None).map_err(payload_error))
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
        Command::SendAttachment {
            peer_id,
            path,
            reply,
        } => {
            let result = proxy
                .send_attachment(&peer_id, &path, "")
                .await
                .map_err(bus_error);
            let _ = reply.send(result);
        }
        Command::AttachmentPath { hash, reply } => {
            let result = proxy.get_attachment_path(&hash).await.map_err(bus_error);
            let _ = reply.send(result);
        }
        Command::SaveAttachment {
            hash,
            destination,
            reply,
        } => {
            let result = proxy
                .save_attachment(&hash, &destination)
                .await
                .map_err(bus_error);
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
        Command::StartCall { peer_id, reply } => {
            let result = proxy.start_call(&peer_id).await.map_err(bus_error);
            let _ = reply.send(result);
        }
        Command::AcceptCall { call_id, reply } => {
            let result = proxy.accept_call(&call_id).await.map_err(bus_error);
            let _ = reply.send(result);
        }
        Command::DeclineCall { call_id, reply } => {
            let result = proxy.decline_call(&call_id).await.map_err(bus_error);
            let _ = reply.send(result);
        }
        Command::HangupCall { call_id, reply } => {
            let result = proxy.hangup_call(&call_id).await.map_err(bus_error);
            let _ = reply.send(result);
        }
        Command::RequestRemoteSession { peer_id, reply } => {
            let result = match proxy.request_remote_session(&peer_id).await {
                Ok(values) => events::parse_remote_session(&values).map_err(payload_error),
                Err(error) => Err(bus_error(error)),
            };
            let _ = reply.send(result);
        }
        Command::AcceptRemoteSession {
            session_id,
            pin,
            reply,
        } => {
            let result = proxy
                .accept_remote_session(&session_id, &pin)
                .await
                .map_err(bus_error);
            let _ = reply.send(result);
        }
        Command::DenyRemoteSession { session_id, reply } => {
            let result = proxy
                .deny_remote_session(&session_id)
                .await
                .map_err(bus_error);
            let _ = reply.send(result);
        }
        Command::EndRemoteSession { session_id, reply } => {
            let result = proxy
                .end_remote_session(&session_id)
                .await
                .map_err(bus_error);
            let _ = reply.send(result);
        }
        Command::GetRemoteSession(reply) => {
            let result = match proxy.get_remote_session().await {
                Ok(values) if values.is_empty() => Ok(None),
                Ok(values) => events::parse_remote_session(&values)
                    .map(Some)
                    .map_err(payload_error),
                Err(error) => Err(bus_error(error)),
            };
            let _ = reply.send(result);
        }
        Command::GetCallState(reply) => {
            let result = match proxy.get_call_state().await {
                Ok(values) if values.is_empty() => Ok(None),
                Ok(values) => events::parse_call(&values).map(Some).map_err(payload_error),
                Err(error) => Err(bus_error(error)),
            };
            let _ = reply.send(result);
        }
    }
}

fn bus_error(error: zbus::Error) -> Error {
    Error::DaemonUnavailable(error.to_string())
}

fn payload_error(error: events::ParseError) -> Error {
    Error::InvalidResponse(error.to_string())
}

fn parse_settings(values: &HashMap<String, OwnedVariant>) -> SettingsForm {
    SettingsForm::new(events::settings_map(values))
}

pub fn parse_activation_view(value: &str) -> Result<&'static str> {
    crate::model::pairing_view(value)
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
        assert!(events::required_string(&values, ipc::PEER_ID).is_err());
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
        assert_eq!(events::display_value(&values["x"]), "true");
        assert_eq!(events::display_value(&values["y"]), "7");
    }
}
