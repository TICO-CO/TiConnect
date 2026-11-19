use std::collections::HashMap;

use futures_util::StreamExt;
use ticonnect_core::ipc;
use ticonnect_core::ipc::client::DaemonProxy;
use ticonnect_core::ipc::events::{
    self, Call, Candidate, ConversationSummary, Message, MessageState, RemoteSession, StorageUsage,
    User, UserPolicy, WatchParty,
};
pub use ticonnect_core::ipc::events::{DaemonEvent, Identity, Status};
use tokio::sync::{mpsc, oneshot};
use zbus::OwnedVariant;

use crate::error::{Error, Result};
use crate::model::{BlockedUser, SettingsForm};

type Reply<T> = oneshot::Sender<Result<T>>;

enum Command {
    GetIdentity(Reply<Identity>),
    GetAvatar {
        fingerprint: String,
        reply: Reply<Vec<u8>>,
    },
    SetAvatar {
        avatar: Vec<u8>,
        reply: Reply<()>,
    },
    ClearAvatar(Reply<()>),
    GetStatus(Reply<Status>),
    ListUsers(Reply<Vec<User>>),
    GetUser {
        user_id: String,
        reply: Reply<ticonnect_core::ipc::events::UserDetails>,
    },
    ListCandidates(Reply<Vec<Candidate>>),
    ListConversationSummaries(Reply<Vec<ConversationSummary>>),
    ListBlockedUsers(Reply<Vec<BlockedUser>>),
    ListSettings(Reply<SettingsForm>),
    BlockUser {
        user_id: String,
        reply: Reply<()>,
    },
    UnblockUser {
        fingerprint: String,
        reply: Reply<()>,
    },
    GetMessage {
        message_id: String,
        reply: Reply<Message>,
    },
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
    ClearHistory {
        user_id: String,
        reply: Reply<u32>,
    },
    StorageUsage(Reply<StorageUsage>),
    SendMessage {
        user_id: String,
        body: String,
        reply: Reply<String>,
    },
    SendAttachment {
        user_id: String,
        path: String,
        mime: String,
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
    MarkConversationRead {
        conversation_id: String,
        reply: Reply<()>,
    },
    SetUserPolicy {
        user_id: String,
        policy: UserPolicy,
        reply: Reply<()>,
    },
    SetTyping {
        user_id: String,
        typing: bool,
        reply: Reply<()>,
    },
    SetDnd {
        enabled: bool,
        reply: Reply<()>,
    },
    StartCall {
        user_id: String,
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
        user_id: String,
        reply: Reply<RemoteSession>,
    },
    AcceptRemoteSession {
        session_id: String,
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
    StartWatchParty {
        user_ids: Vec<String>,
        video_id: String,
        reply: Reply<WatchParty>,
    },
    InviteWatchParty {
        party_id: String,
        user_ids: Vec<String>,
        reply: Reply<WatchParty>,
    },
    AcceptWatchParty {
        party_id: String,
        reply: Reply<WatchParty>,
    },
    DeclineWatchParty {
        party_id: String,
        reply: Reply<()>,
    },
    LeaveWatchParty {
        party_id: String,
        reply: Reply<()>,
    },
    PublishWatchParty {
        party_id: String,
        position_ms: u64,
        playing: bool,
        reply: Reply<WatchParty>,
    },
    EndWatchParty {
        party_id: String,
        reply: Reply<()>,
    },
    GetWatchParty(Reply<Option<WatchParty>>),
}

#[derive(Clone)]
pub struct LinkHandle {
    commands: mpsc::UnboundedSender<Command>,
}

impl LinkHandle {
    #[doc(hidden)]
    pub fn offline() -> Self {
        let (commands, receiver) = mpsc::unbounded_channel();
        drop(receiver);
        Self { commands }
    }

    pub async fn identity(&self) -> Result<Identity> {
        self.request(Command::GetIdentity).await
    }

    pub async fn get_avatar(&self, fingerprint: String) -> Result<Vec<u8>> {
        self.request(|reply| Command::GetAvatar { fingerprint, reply })
            .await
    }

    pub async fn set_avatar(&self, avatar: Vec<u8>) -> Result<()> {
        self.request(|reply| Command::SetAvatar { avatar, reply })
            .await
    }

    pub async fn clear_avatar(&self) -> Result<()> {
        self.request(Command::ClearAvatar).await
    }

    pub async fn list_conversation_summaries(&self) -> Result<Vec<ConversationSummary>> {
        self.request(Command::ListConversationSummaries).await
    }

    pub async fn status(&self) -> Result<Status> {
        self.request(Command::GetStatus).await
    }

    pub async fn list_users(&self) -> Result<Vec<User>> {
        self.request(Command::ListUsers).await
    }

    pub async fn get_user(
        &self,
        user_id: String,
    ) -> Result<ticonnect_core::ipc::events::UserDetails> {
        self.request(|reply| Command::GetUser { user_id, reply })
            .await
    }

    pub async fn list_candidates(&self) -> Result<Vec<Candidate>> {
        self.request(Command::ListCandidates).await
    }

    pub async fn list_blocked_users(&self) -> Result<Vec<BlockedUser>> {
        self.request(Command::ListBlockedUsers).await
    }

    pub async fn list_settings(&self) -> Result<SettingsForm> {
        self.request(Command::ListSettings).await
    }

    pub async fn block_user(&self, user_id: String) -> Result<()> {
        self.request(|reply| Command::BlockUser { user_id, reply })
            .await
    }

    pub async fn unblock_user(&self, fingerprint: String) -> Result<()> {
        self.request(|reply| Command::UnblockUser { fingerprint, reply })
            .await
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

    pub async fn get_message(&self, message_id: String) -> Result<Message> {
        self.request(|reply| Command::GetMessage { message_id, reply })
            .await
    }

    pub async fn count_messages(&self, conversation_id: String) -> Result<u32> {
        self.request(|reply| Command::CountMessages {
            conversation_id,
            reply,
        })
        .await
    }

    pub async fn clear_history(&self, user_id: String) -> Result<u32> {
        self.request(|reply| Command::ClearHistory { user_id, reply })
            .await
    }

    pub async fn storage_usage(&self) -> Result<StorageUsage> {
        self.request(Command::StorageUsage).await
    }

    pub async fn send_message(&self, user_id: String, body: String) -> Result<String> {
        self.request(|reply| Command::SendMessage {
            user_id,
            body,
            reply,
        })
        .await
    }

    pub async fn send_attachment(
        &self,
        user_id: String,
        path: String,
        mime: String,
    ) -> Result<String> {
        self.request(|reply| Command::SendAttachment {
            user_id,
            path,
            mime,
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

    pub async fn mark_conversation_read(&self, conversation_id: String) -> Result<()> {
        self.request(|reply| Command::MarkConversationRead {
            conversation_id,
            reply,
        })
        .await
    }

    pub async fn set_user_policy(&self, user_id: String, policy: UserPolicy) -> Result<()> {
        self.request(|reply| Command::SetUserPolicy {
            user_id,
            policy,
            reply,
        })
        .await
    }

    pub async fn set_typing(&self, user_id: String, typing: bool) -> Result<()> {
        self.request(|reply| Command::SetTyping {
            user_id,
            typing,
            reply,
        })
        .await
    }

    pub async fn set_dnd(&self, enabled: bool) -> Result<()> {
        self.request(|reply| Command::SetDnd { enabled, reply })
            .await
    }

    pub async fn start_call(&self, user_id: String) -> Result<String> {
        self.request(|reply| Command::StartCall { user_id, reply })
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

    pub async fn request_remote_session(&self, user_id: String) -> Result<RemoteSession> {
        self.request(|reply| Command::RequestRemoteSession { user_id, reply })
            .await
    }

    pub async fn accept_remote_session(&self, session_id: String) -> Result<()> {
        self.request(|reply| Command::AcceptRemoteSession { session_id, reply })
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

    pub async fn start_watch_party(
        &self,
        user_ids: Vec<String>,
        video_id: String,
    ) -> Result<WatchParty> {
        self.request(|reply| Command::StartWatchParty {
            user_ids,
            video_id,
            reply,
        })
        .await
    }

    pub async fn invite_watch_party(
        &self,
        party_id: String,
        user_ids: Vec<String>,
    ) -> Result<WatchParty> {
        self.request(|reply| Command::InviteWatchParty {
            party_id,
            user_ids,
            reply,
        })
        .await
    }

    pub async fn accept_watch_party(&self, party_id: String) -> Result<WatchParty> {
        self.request(|reply| Command::AcceptWatchParty { party_id, reply })
            .await
    }

    pub async fn decline_watch_party(&self, party_id: String) -> Result<()> {
        self.request(|reply| Command::DeclineWatchParty { party_id, reply })
            .await
    }

    pub async fn leave_watch_party(&self, party_id: String) -> Result<()> {
        self.request(|reply| Command::LeaveWatchParty { party_id, reply })
            .await
    }

    pub async fn publish_watch_party(
        &self,
        party_id: String,
        position_ms: u64,
        playing: bool,
    ) -> Result<WatchParty> {
        self.request(|reply| Command::PublishWatchParty {
            party_id,
            position_ms,
            playing,
            reply,
        })
        .await
    }

    pub async fn end_watch_party(&self, party_id: String) -> Result<()> {
        self.request(|reply| Command::EndWatchParty { party_id, reply })
            .await
    }

    pub async fn watch_party(&self) -> Result<Option<WatchParty>> {
        self.request(Command::GetWatchParty).await
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
    tokio::spawn(run_link(receiver, events));
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
                crate::timing::mark("daemon link connected");
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
    let connection = ipc::connect().await.map_err(bus_error)?;
    let proxy = DaemonProxy::new(&connection).await.map_err(bus_error)?;
    proxy.ping().await.map_err(bus_error)?;
    Ok((connection, proxy))
}

fn spawn_listener(
    connection: zbus::Connection,
    events: mpsc::UnboundedSender<DaemonEvent>,
    lost: mpsc::UnboundedSender<String>,
) {
    tokio::spawn(async move {
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
    let mut conversation_changed = proxy
        .receive_conversation_changed()
        .await
        .map_err(bus_error)?;
    let mut typing_changed = proxy.receive_typing_changed().await.map_err(bus_error)?;
    let mut user_profile_changed = proxy
        .receive_user_profile_changed()
        .await
        .map_err(bus_error)?;
    let mut self_profile_changed = proxy
        .receive_self_profile_changed()
        .await
        .map_err(bus_error)?;
    let mut connected = proxy.receive_user_connected().await.map_err(bus_error)?;
    let mut disconnected = proxy.receive_user_disconnected().await.map_err(bus_error)?;
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
    let mut watch_party_requested = proxy
        .receive_watch_party_requested()
        .await
        .map_err(bus_error)?;
    let mut watch_party_changed = proxy
        .receive_watch_party_changed()
        .await
        .map_err(bus_error)?;
    let mut watch_party_ended = proxy.receive_watch_party_ended().await.map_err(bus_error)?;
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
                let Ok(args) = signal.args() else {
                    tracing::warn!("failed to read daemon signal arguments");
                    continue;
                };
                if let Some(message) =
                    parse_event(events::parse_message(&args.message, None), "message_received")
                {
                    let _ = events.send(DaemonEvent::MessageReceived(message));
                }
            }
            signal = states.next() => {
                let Some(signal) = signal else { break };
                let Ok(args) = signal.args() else {
                    tracing::warn!("failed to read daemon signal arguments");
                    continue;
                };
                let Some(state) =
                    parse_event(MessageState::parse(args.state), "message_state_changed")
                else {
                    continue;
                };
                let _ = events.send(DaemonEvent::MessageStateChanged {
                    message_id: args.message_id.to_owned(),
                    state,
                });
            }
            signal = conversation_changed.next() => {
                let Some(signal) = signal else { break };
                let Ok(args) = signal.args() else {
                    tracing::warn!("failed to read daemon signal arguments");
                    continue;
                };
                let _ = events.send(DaemonEvent::ConversationChanged {
                    user_id: args.user_id.to_owned(),
                });
            }
            signal = typing_changed.next() => {
                let Some(signal) = signal else { break };
                let Ok(args) = signal.args() else {
                    tracing::warn!("failed to read daemon signal arguments");
                    continue;
                };
                let _ = events.send(DaemonEvent::TypingChanged {
                    user_id: args.user_id.to_owned(),
                    typing: args.typing,
                });
            }
            signal = user_profile_changed.next() => {
                let Some(signal) = signal else { break };
                let Ok(args) = signal.args() else {
                    tracing::warn!("failed to read daemon signal arguments");
                    continue;
                };
                let _ = events.send(DaemonEvent::UserProfileChanged {
                    fingerprint: args.fingerprint.to_owned(),
                });
            }
            signal = self_profile_changed.next() => {
                let Some(_signal) = signal else { break };
                let _ = events.send(DaemonEvent::SelfProfileChanged);
            }
            signal = connected.next() => {
                let Some(signal) = signal else { break };
                let Ok(args) = signal.args() else {
                    tracing::warn!("failed to read daemon signal arguments");
                    continue;
                };
                if let Some(user) =
                    parse_event(events::parse_user(&args.user), "user_connected")
                {
                    let _ = events.send(DaemonEvent::UserConnected(user));
                }
            }
            signal = disconnected.next() => {
                let Some(signal) = signal else { break };
                let Ok(args) = signal.args() else {
                    tracing::warn!("failed to read daemon signal arguments");
                    continue;
                };
                if let Some(user) =
                    parse_event(events::parse_user(&args.user), "user_disconnected")
                {
                    let _ = events.send(DaemonEvent::UserDisconnected(user));
                }
            }
            signal = policy_changed.next() => {
                let Some(signal) = signal else { break };
                let Ok(args) = signal.args() else {
                    tracing::warn!("failed to read daemon signal arguments");
                    continue;
                };
                let Some(policy) = parse_event(UserPolicy::parse(args.policy), "policy_changed")
                else {
                    continue;
                };
                let _ = events.send(DaemonEvent::PolicyChanged {
                    user_id: args.user_id.to_owned(),
                    policy,
                });
            }
            signal = settings_changed.next() => {
                let Some(signal) = signal else { break };
                let Ok(args) = signal.args() else {
                    tracing::warn!("failed to read daemon signal arguments");
                    continue;
                };
                let _ = events.send(DaemonEvent::SettingsChanged(args.key.to_owned()));
            }
            signal = incoming_call.next() => {
                let Some(signal) = signal else { break };
                let Ok(args) = signal.args() else {
                    tracing::warn!("failed to read daemon signal arguments");
                    continue;
                };
                if let Some(call) =
                    parse_event(events::parse_call(&args.call), "incoming_call")
                {
                    let _ = events.send(DaemonEvent::IncomingCall(call));
                }
            }
            signal = call_state_changed.next() => {
                let Some(signal) = signal else { break };
                let Ok(args) = signal.args() else {
                    tracing::warn!("failed to read daemon signal arguments");
                    continue;
                };
                let _ = events.send(DaemonEvent::CallStateChanged {
                    call_id: args.call_id.to_owned(),
                    state: args.state.to_owned(),
                });
            }
            signal = call_ended.next() => {
                let Some(signal) = signal else { break };
                let Ok(args) = signal.args() else {
                    tracing::warn!("failed to read daemon signal arguments");
                    continue;
                };
                let _ = events.send(DaemonEvent::CallEnded {
                    call_id: args.call_id.to_owned(),
                    reason: args.reason.to_owned(),
                });
            }
            signal = remote_requested.next() => {
                let Some(signal) = signal else { break };
                let Ok(args) = signal.args() else {
                    tracing::warn!("failed to read daemon signal arguments");
                    continue;
                };
                if let Some(session) = parse_event(
                    events::parse_remote_session(&args.session),
                    "remote_session_requested",
                ) {
                    let _ = events.send(DaemonEvent::RemoteSessionRequested(session));
                }
            }
            signal = remote_started.next() => {
                let Some(signal) = signal else { break };
                let Ok(args) = signal.args() else {
                    tracing::warn!("failed to read daemon signal arguments");
                    continue;
                };
                let _ = events.send(DaemonEvent::RemoteSessionStarted(args.session_id.to_owned()));
            }
            signal = remote_ended.next() => {
                let Some(signal) = signal else { break };
                let Ok(args) = signal.args() else {
                    tracing::warn!("failed to read daemon signal arguments");
                    continue;
                };
                let _ = events.send(DaemonEvent::RemoteSessionEnded {
                    session_id: args.session_id.to_owned(),
                    reason: args.reason.to_owned(),
                });
            }
            signal = remote_indicator.next() => {
                let Some(signal) = signal else { break };
                let Ok(args) = signal.args() else {
                    tracing::warn!("failed to read daemon signal arguments");
                    continue;
                };
                let _ = events.send(DaemonEvent::RemoteIndicatorChanged {
                    session_id: args.session_id.to_owned(),
                    visible: args.visible,
                });
            }
            signal = transfer_progress.next() => {
                let Some(signal) = signal else { break };
                let Ok(args) = signal.args() else {
                    tracing::warn!("failed to read daemon signal arguments");
                    continue;
                };
                let _ = events.send(DaemonEvent::TransferProgress {
                    hash: args.hash.to_owned(),
                    bytes: args.bytes,
                    total: args.total,
                });
            }
            signal = watch_party_requested.next() => {
                let Some(signal) = signal else { break };
                let Ok(args) = signal.args() else {
                    tracing::warn!("failed to read daemon signal arguments");
                    continue;
                };
                if let Some(party) = parse_event(
                    events::parse_watch_party(&args.party),
                    "watch_party_requested",
                ) {
                    let _ = events.send(DaemonEvent::WatchPartyRequested(party));
                }
            }
            signal = watch_party_changed.next() => {
                let Some(signal) = signal else { break };
                let Ok(args) = signal.args() else {
                    tracing::warn!("failed to read daemon signal arguments");
                    continue;
                };
                if let Some(party) = parse_event(
                    events::parse_watch_party(&args.party),
                    "watch_party_changed",
                ) {
                    let _ = events.send(DaemonEvent::WatchPartyChanged(party));
                }
            }
            signal = watch_party_ended.next() => {
                let Some(signal) = signal else { break };
                let Ok(args) = signal.args() else {
                    tracing::warn!("failed to read daemon signal arguments");
                    continue;
                };
                let _ = events.send(DaemonEvent::WatchPartyEnded {
                    party_id: args.party_id.to_owned(),
                    reason: args.reason.to_owned(),
                });
            }
            signal = transfer_done.next() => {
                let Some(signal) = signal else { break };
                let Ok(args) = signal.args() else {
                    tracing::warn!("failed to read daemon signal arguments");
                    continue;
                };
                let _ = events.send(DaemonEvent::TransferDone(args.hash.to_owned()));
            }
            signal = transfer_failed.next() => {
                let Some(signal) = signal else { break };
                let Ok(args) = signal.args() else {
                    tracing::warn!("failed to read daemon signal arguments");
                    continue;
                };
                let _ = events.send(DaemonEvent::TransferFailed {
                    hash: args.hash.to_owned(),
                    reason: args.reason.to_owned(),
                });
            }
            signal = sync_completed.next() => {
                let Some(signal) = signal else { break };
                let Ok(args) = signal.args() else {
                    tracing::warn!("failed to read daemon signal arguments");
                    continue;
                };
                let _ = events.send(DaemonEvent::SyncCompleted {
                    user_id: args.user_id.to_owned(),
                    count: args.count,
                });
            }
            signal = candidate_found.next() => {
                let Some(signal) = signal else { break };
                let Ok(args) = signal.args() else {
                    tracing::warn!("failed to read daemon signal arguments");
                    continue;
                };
                if let Some(candidate) = parse_event(
                    events::parse_candidate(&args.candidate),
                    "candidate_found",
                ) {
                    let _ = events.send(DaemonEvent::Candidates(vec![candidate]));
                }
            }
            signal = candidate_lost.next() => {
                let Some(signal) = signal else { break };
                let Ok(args) = signal.args() else {
                    tracing::warn!("failed to read daemon signal arguments");
                    continue;
                };
                let _ = events.send(DaemonEvent::CandidateLost(args.fingerprint.to_owned()));
                tracing::debug!(fingerprint = %args.fingerprint, "candidate lost");
            }
            signal = owner_changed.next() => {
                let Some(signal) = signal else { break };
                let Ok(args) = signal.args() else {
                    tracing::warn!("failed to read daemon signal arguments");
                    continue;
                };
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
        Command::GetAvatar { fingerprint, reply } => {
            let result = proxy.get_avatar(&fingerprint).await.map_err(bus_error);
            let _ = reply.send(result);
        }
        Command::SetAvatar { avatar, reply } => {
            let result = proxy.set_avatar(&avatar).await.map_err(bus_error);
            let _ = reply.send(result);
        }
        Command::ClearAvatar(reply) => {
            let result = proxy.clear_avatar().await.map_err(bus_error);
            let _ = reply.send(result);
        }
        Command::GetStatus(reply) => {
            let result = match proxy.get_status().await {
                Ok(values) => events::parse_status(&values).map_err(payload_error),
                Err(error) => Err(bus_error(error)),
            };
            let _ = reply.send(result);
        }
        Command::ListUsers(reply) => {
            let result = match proxy.list_users().await {
                Ok(values) => events::parse_users(&values).map_err(payload_error),
                Err(error) => Err(bus_error(error)),
            };
            let _ = reply.send(result);
        }
        Command::GetUser { user_id, reply } => {
            let result = match proxy.get_user(&user_id).await {
                Ok(values) => events::parse_user_details(&values).map_err(payload_error),
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
        Command::ListConversationSummaries(reply) => {
            let result = match proxy.list_conversation_summaries().await {
                Ok(values) => values
                    .iter()
                    .map(|values| events::parse_conversation_summary(values).map_err(payload_error))
                    .collect::<Result<Vec<_>>>(),
                Err(error) => Err(bus_error(error)),
            };
            let _ = reply.send(result);
        }
        Command::ListSettings(reply) => {
            let result = match proxy.list_settings().await {
                Ok(values) => parse_settings(&values),
                Err(error) => Err(bus_error(error)),
            };
            let _ = reply.send(result);
        }
        Command::ListBlockedUsers(reply) => {
            let result = match proxy.list_blocked_users().await {
                Ok(values) => parse_blocked_users(&values),
                Err(error) => Err(bus_error(error)),
            };
            let _ = reply.send(result);
        }
        Command::BlockUser { user_id, reply } => {
            let result = proxy
                .forget_user(&user_id)
                .await
                .map(|_| ())
                .map_err(bus_error);
            let _ = reply.send(result);
        }
        Command::UnblockUser { fingerprint, reply } => {
            let result = proxy
                .unblock_user(&fingerprint)
                .await
                .map(|_| ())
                .map_err(bus_error);
            let _ = reply.send(result);
        }
        Command::GetMessage { message_id, reply } => {
            let result = match proxy.get_message(&message_id).await {
                Ok(values) => events::parse_message(&values, None).map_err(payload_error),
                Err(error) => Err(bus_error(error)),
            };
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
        Command::ClearHistory { user_id, reply } => {
            let result = proxy.clear_history(&user_id).await.map_err(bus_error);
            let _ = reply.send(result);
        }
        Command::StorageUsage(reply) => {
            let result = match proxy.get_storage_usage().await {
                Ok(values) => events::parse_storage_usage(&values).map_err(payload_error),
                Err(error) => Err(bus_error(error)),
            };
            let _ = reply.send(result);
        }
        Command::SendMessage {
            user_id,
            body,
            reply,
        } => {
            let result = proxy.send_message(&user_id, &body).await.map_err(bus_error);
            let _ = reply.send(result);
        }
        Command::SendAttachment {
            user_id,
            path,
            mime,
            reply,
        } => {
            let result = proxy
                .send_attachment(&user_id, &path, &mime)
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
        Command::MarkConversationRead {
            conversation_id,
            reply,
        } => {
            let result = proxy
                .mark_conversation_read(&conversation_id)
                .await
                .map_err(bus_error);
            let _ = reply.send(result);
        }
        Command::SetUserPolicy {
            user_id,
            policy,
            reply,
        } => {
            let result = proxy
                .set_user_policy(&user_id, policy.as_str())
                .await
                .map_err(bus_error);
            let _ = reply.send(result);
        }
        Command::SetTyping {
            user_id,
            typing,
            reply,
        } => {
            let result = proxy.set_typing(&user_id, typing).await.map_err(bus_error);
            let _ = reply.send(result);
        }
        Command::SetDnd { enabled, reply } => {
            let result = proxy.set_dnd(enabled).await.map_err(bus_error);
            let _ = reply.send(result);
        }
        Command::StartCall { user_id, reply } => {
            let result = proxy.start_call(&user_id).await.map_err(bus_error);
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
        Command::StartWatchParty {
            user_ids,
            video_id,
            reply,
        } => {
            let ids: Vec<&str> = user_ids.iter().map(String::as_str).collect();
            let result = match proxy.start_watch_party(&ids, &video_id).await {
                Ok(values) => events::parse_watch_party(&values).map_err(payload_error),
                Err(error) => Err(bus_error(error)),
            };
            let _ = reply.send(result);
        }
        Command::InviteWatchParty {
            party_id,
            user_ids,
            reply,
        } => {
            let ids: Vec<&str> = user_ids.iter().map(String::as_str).collect();
            let result = match proxy.invite_watch_party(&party_id, &ids).await {
                Ok(values) => events::parse_watch_party(&values).map_err(payload_error),
                Err(error) => Err(bus_error(error)),
            };
            let _ = reply.send(result);
        }
        Command::AcceptWatchParty { party_id, reply } => {
            let result = match proxy.accept_watch_party(&party_id).await {
                Ok(values) => events::parse_watch_party(&values).map_err(payload_error),
                Err(error) => Err(bus_error(error)),
            };
            let _ = reply.send(result);
        }
        Command::DeclineWatchParty { party_id, reply } => {
            let result = proxy
                .decline_watch_party(&party_id)
                .await
                .map_err(bus_error);
            let _ = reply.send(result);
        }
        Command::LeaveWatchParty { party_id, reply } => {
            let result = proxy.leave_watch_party(&party_id).await.map_err(bus_error);
            let _ = reply.send(result);
        }
        Command::PublishWatchParty {
            party_id,
            position_ms,
            playing,
            reply,
        } => {
            let result = match proxy
                .publish_watch_party(&party_id, position_ms, playing)
                .await
            {
                Ok(values) => events::parse_watch_party(&values).map_err(payload_error),
                Err(error) => Err(bus_error(error)),
            };
            let _ = reply.send(result);
        }
        Command::EndWatchParty { party_id, reply } => {
            let result = proxy.end_watch_party(&party_id).await.map_err(bus_error);
            let _ = reply.send(result);
        }
        Command::GetWatchParty(reply) => {
            let result = match proxy.get_watch_party().await {
                Ok(values) if values.is_empty() => Ok(None),
                Ok(values) => events::parse_watch_party(&values)
                    .map(Some)
                    .map_err(payload_error),
                Err(error) => Err(bus_error(error)),
            };
            let _ = reply.send(result);
        }
        Command::RequestRemoteSession { user_id, reply } => {
            let result = match proxy.request_remote_session(&user_id).await {
                Ok(values) => events::parse_remote_session(&values).map_err(payload_error),
                Err(error) => Err(bus_error(error)),
            };
            let _ = reply.send(result);
        }
        Command::AcceptRemoteSession { session_id, reply } => {
            let result = proxy
                .accept_remote_session(&session_id)
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

fn parse_settings(values: &HashMap<String, OwnedVariant>) -> Result<SettingsForm> {
    events::settings_map(values)
        .map(SettingsForm::new)
        .map_err(payload_error)
}

fn parse_blocked_users(values: &[HashMap<String, OwnedVariant>]) -> Result<Vec<BlockedUser>> {
    values
        .iter()
        .map(|user| {
            Ok(BlockedUser {
                fingerprint: events::required_string(user, ipc::USER_FINGERPRINT)?,
                node_id: events::required_string(user, ipc::USER_ID)?,
                display_name: events::optional_string(user, ipc::USER_DISPLAY_NAME)?,
                blocked_at: events::optional_u64(user, ipc::BLOCKED_AT)?,
            })
        })
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(payload_error)
}

fn parse_event<T>(result: events::Result<T>, name: &'static str) -> Option<T> {
    match result {
        Ok(value) => Some(value),
        Err(error) => {
            tracing::warn!(%error, %name, "failed to parse daemon event");
            None
        }
    }
}

pub fn parse_activation_view(value: &str) -> Result<&'static str> {
    crate::model::activation_view(value)
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
        assert!(events::required_string(&values, ipc::USER_ID).is_err());
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
        assert_eq!(
            events::display_value(&values["x"], "x").expect("bool"),
            "true"
        );
        assert_eq!(
            events::display_value(&values["y"], "y").expect("integer"),
            "7"
        );
    }
}
