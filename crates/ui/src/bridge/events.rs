use serde::Serialize;
use ticonnect_core::ipc::events::{
    Call, Candidate, DaemonEvent, Message, MessageState, PairingStart, Peer, PeerPolicy,
    RemoteSession,
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
        peer_id: Option<String>,
    },
    Connected,
    Disconnected {
        reason: String,
    },
    Peers {
        peers: Vec<Peer>,
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
    ConversationChanged {
        peer_id: String,
    },
    TypingChanged {
        peer_id: String,
        typing: bool,
    },
    PeerProfileChanged {
        fingerprint: String,
    },
    SelfProfileChanged,
    PeerConnected {
        peer: Peer,
    },
    PeerDisconnected {
        peer: Peer,
    },
    PairingStarted {
        pairing: PairingStart,
    },
    PairingCompleted {
        peer_id: String,
        fingerprint: String,
    },
    PairingFailed {
        reason: String,
    },
    PolicyChanged {
        peer_id: String,
        policy: PeerPolicy,
    },
    SettingsChanged {
        key: String,
    },
    SyncCompleted {
        peer_id: String,
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
            DaemonEvent::Peers(peers) => Self::Peers {
                peers: peers.clone(),
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
            DaemonEvent::ConversationChanged { peer_id } => Self::ConversationChanged {
                peer_id: peer_id.clone(),
            },
            DaemonEvent::TypingChanged { peer_id, typing } => Self::TypingChanged {
                peer_id: peer_id.clone(),
                typing: *typing,
            },
            DaemonEvent::PeerProfileChanged { fingerprint } => Self::PeerProfileChanged {
                fingerprint: fingerprint.clone(),
            },
            DaemonEvent::SelfProfileChanged => Self::SelfProfileChanged,
            DaemonEvent::PeerConnected(peer) => Self::PeerConnected { peer: peer.clone() },
            DaemonEvent::PeerDisconnected(peer) => Self::PeerDisconnected { peer: peer.clone() },
            DaemonEvent::PairingStarted(pairing) => Self::PairingStarted {
                pairing: pairing.clone(),
            },
            DaemonEvent::PairingCompleted {
                peer_id,
                fingerprint,
            } => Self::PairingCompleted {
                peer_id: peer_id.clone(),
                fingerprint: fingerprint.clone(),
            },
            DaemonEvent::PairingFailed(reason) => Self::PairingFailed {
                reason: reason.clone(),
            },
            DaemonEvent::PolicyChanged { peer_id, policy } => Self::PolicyChanged {
                peer_id: peer_id.clone(),
                policy: *policy,
            },
            DaemonEvent::SettingsChanged(key) => Self::SettingsChanged { key: key.clone() },
            DaemonEvent::SyncCompleted { peer_id, count } => Self::SyncCompleted {
                peer_id: peer_id.clone(),
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
            DaemonEvent::CallEnded { call_id, reason } => Self::CallEnded {
                call_id: call_id.clone(),
                reason: reason.clone(),
            },
            DaemonEvent::RemoteSessionRequested(session) => Self::RemoteSessionRequested {
                session: session.clone(),
            },
            DaemonEvent::RemoteSessionStarted(session_id) => Self::RemoteSessionStarted {
                session_id: session_id.clone(),
            },
            DaemonEvent::RemoteSessionEnded { session_id, reason } => Self::RemoteSessionEnded {
                session_id: session_id.clone(),
                reason: reason.clone(),
            },
            DaemonEvent::RemoteIndicatorChanged {
                session_id,
                visible,
            } => Self::RemoteIndicatorChanged {
                session_id: session_id.clone(),
                visible: *visible,
            },
        }
    }

    pub fn from_activation(event: &ActivationEvent) -> Self {
        match event {
            ActivationEvent::Primary => Self::Primary,
            ActivationEvent::Duplicate => Self::Duplicate,
            ActivationEvent::Show(view) => Self::Activate {
                view: view.clone(),
                peer_id: None,
            },
            ActivationEvent::ShowPeer(peer_id) => Self::Activate {
                view: "chat".to_owned(),
                peer_id: Some(peer_id.clone()),
            },
        }
    }

    pub fn encode(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| {
            r#"{"type":"error","code":"serialize","message":"serialize event"}"#.to_owned()
        })
    }
}
