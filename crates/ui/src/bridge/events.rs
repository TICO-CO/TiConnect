use serde::Serialize;
use ticonnect_core::ipc::events::{
    Call, Candidate, DaemonEvent, Message, MessageState, RemoteSession, User, UserPolicy,
    WatchParty,
};

use crate::activation::ActivationEvent;

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WsEvent {
    Ready,
    Primary,
    Duplicate,
    Activate {
        view: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        user_id: Option<String>,
    },
    Connected,
    Disconnected {
        reason: String,
    },
    Users {
        users: Vec<User>,
    },
    Candidates {
        candidates: Vec<Candidate>,
    },
    CandidateLost {
        fingerprint: String,
    },
    MessageReceived {
        message: Message,
    },
    MessageStateChanged {
        message_id: String,
        state: MessageState,
    },
    MessageEdited {
        message_id: String,
        user_id: String,
        body: String,
        edited_at: u64,
    },
    MessageDeleted {
        message_id: String,
        user_id: String,
    },
    ConversationChanged {
        user_id: String,
    },
    TypingChanged {
        user_id: String,
        typing: bool,
    },
    UserProfileChanged {
        fingerprint: String,
    },
    SelfProfileChanged,
    UserConnected {
        user: User,
    },
    UserDisconnected {
        user: User,
    },
    PolicyChanged {
        user_id: String,
        policy: UserPolicy,
    },
    SettingsChanged {
        key: String,
    },
    SyncCompleted {
        user_id: String,
        count: u32,
    },
    TransferProgress {
        hash: String,
        bytes: u64,
        total: u64,
    },
    TransferDone {
        hash: String,
    },
    TransferFailed {
        hash: String,
        reason: String,
    },
    IncomingCall {
        call: Call,
    },
    CallStateChanged {
        call_id: String,
        state: String,
    },
    CallMuteChanged {
        call_id: String,
        input_muted: bool,
        output_muted: bool,
    },
    CallEnded {
        call_id: String,
        reason: String,
    },
    RemoteSessionRequested {
        session: RemoteSession,
    },
    RemoteSessionStarted {
        session_id: String,
    },
    RemoteSessionEnded {
        session_id: String,
        reason: String,
    },
    RemoteIndicatorChanged {
        session_id: String,
        visible: bool,
    },
    RemoteInputChanged {
        session_id: String,
        allowed: bool,
    },
    WatchPartyRequested {
        party: WatchParty,
    },
    WatchPartyChanged {
        party: WatchParty,
    },
    WatchPartyEnded {
        party_id: String,
        reason: String,
    },
    Error {
        code: String,
        message: String,
    },
}

impl WsEvent {
    pub fn from_daemon(event: &DaemonEvent) -> Self {
        match event {
            DaemonEvent::Connected => Self::Connected,
            DaemonEvent::Disconnected(reason) => Self::Disconnected {
                reason: reason.clone(),
            },
            DaemonEvent::Users(users) => Self::Users {
                users: users.clone(),
            },
            DaemonEvent::Candidates(candidates) => Self::Candidates {
                candidates: candidates.clone(),
            },
            DaemonEvent::CandidateLost(fingerprint) => Self::CandidateLost {
                fingerprint: fingerprint.clone(),
            },
            DaemonEvent::MessageReceived(message) => Self::MessageReceived {
                message: message.clone(),
            },
            DaemonEvent::MessageStateChanged { message_id, state } => Self::MessageStateChanged {
                message_id: message_id.clone(),
                state: *state,
            },
            DaemonEvent::MessageEdited {
                message_id,
                user_id,
                body,
                edited_at,
            } => Self::MessageEdited {
                message_id: message_id.clone(),
                user_id: user_id.clone(),
                body: body.clone(),
                edited_at: *edited_at,
            },
            DaemonEvent::MessageDeleted {
                message_id,
                user_id,
            } => Self::MessageDeleted {
                message_id: message_id.clone(),
                user_id: user_id.clone(),
            },
            DaemonEvent::ConversationChanged { user_id } => Self::ConversationChanged {
                user_id: user_id.clone(),
            },
            DaemonEvent::TypingChanged { user_id, typing } => Self::TypingChanged {
                user_id: user_id.clone(),
                typing: *typing,
            },
            DaemonEvent::UserProfileChanged { fingerprint } => Self::UserProfileChanged {
                fingerprint: fingerprint.clone(),
            },
            DaemonEvent::SelfProfileChanged => Self::SelfProfileChanged,
            DaemonEvent::UserConnected(user) => Self::UserConnected { user: user.clone() },
            DaemonEvent::UserDisconnected(user) => Self::UserDisconnected { user: user.clone() },
            DaemonEvent::PolicyChanged { user_id, policy } => Self::PolicyChanged {
                user_id: user_id.clone(),
                policy: *policy,
            },
            DaemonEvent::SettingsChanged(key) => Self::SettingsChanged { key: key.clone() },
            DaemonEvent::SyncCompleted { user_id, count } => Self::SyncCompleted {
                user_id: user_id.clone(),
                count: *count,
            },
            DaemonEvent::TransferProgress { hash, bytes, total } => Self::TransferProgress {
                hash: hash.clone(),
                bytes: *bytes,
                total: *total,
            },
            DaemonEvent::TransferDone(hash) => Self::TransferDone { hash: hash.clone() },
            DaemonEvent::TransferFailed { hash, reason } => Self::TransferFailed {
                hash: hash.clone(),
                reason: reason.clone(),
            },
            DaemonEvent::IncomingCall(call) => Self::IncomingCall { call: call.clone() },
            DaemonEvent::CallStateChanged { call_id, state } => Self::CallStateChanged {
                call_id: call_id.clone(),
                state: state.clone(),
            },
            DaemonEvent::CallMuteChanged {
                call_id,
                input_muted,
                output_muted,
            } => Self::CallMuteChanged {
                call_id: call_id.clone(),
                input_muted: *input_muted,
                output_muted: *output_muted,
            },
            DaemonEvent::CallEnded { call_id, reason } => Self::CallEnded {
                call_id: call_id.clone(),
                reason: reason.as_str().to_owned(),
            },
            DaemonEvent::RemoteSessionRequested(session) => Self::RemoteSessionRequested {
                session: session.clone(),
            },
            DaemonEvent::RemoteSessionStarted(session_id) => Self::RemoteSessionStarted {
                session_id: session_id.clone(),
            },
            DaemonEvent::RemoteSessionEnded { session_id, reason } => Self::RemoteSessionEnded {
                session_id: session_id.clone(),
                reason: reason.as_str().to_owned(),
            },
            DaemonEvent::RemoteIndicatorChanged {
                session_id,
                visible,
            } => Self::RemoteIndicatorChanged {
                session_id: session_id.clone(),
                visible: *visible,
            },
            DaemonEvent::RemoteInputChanged {
                session_id,
                allowed,
            } => Self::RemoteInputChanged {
                session_id: session_id.clone(),
                allowed: *allowed,
            },
            DaemonEvent::WatchPartyRequested(party) => Self::WatchPartyRequested {
                party: party.clone(),
            },
            DaemonEvent::WatchPartyChanged(party) => Self::WatchPartyChanged {
                party: party.clone(),
            },
            DaemonEvent::WatchPartyEnded { party_id, reason } => Self::WatchPartyEnded {
                party_id: party_id.clone(),
                reason: reason.as_str().to_owned(),
            },
        }
    }

    pub fn from_activation(event: &ActivationEvent) -> Self {
        match event {
            ActivationEvent::Primary => Self::Primary,
            ActivationEvent::Duplicate => Self::Duplicate,
            ActivationEvent::Show(view) => Self::Activate {
                view: view.clone(),
                user_id: None,
            },
            ActivationEvent::ShowUser(user_id) => Self::Activate {
                view: "chat".to_owned(),
                user_id: Some(user_id.clone()),
            },
        }
    }

    pub fn encode(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| {
            r#"{"type":"error","code":"serialize","message":"serialize event"}"#.to_owned()
        })
    }
}
