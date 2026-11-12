use std::collections::{BTreeMap, HashMap};

use thiserror::Error;
use zbus::OwnedVariant;

use super::{
    CANDIDATE_ADDRESSES, CANDIDATE_DISPLAY_NAME, CANDIDATE_FINGERPRINT, CANDIDATE_NODE_ID,
    CANDIDATE_SOURCE, IDENTITY_DISPLAY_NAME, IDENTITY_FINGERPRINT, IDENTITY_NODE_ID, MESSAGE_BODY,
    MESSAGE_CONVERSATION_ID, MESSAGE_ID, MESSAGE_NOTIFY, MESSAGE_PEER_ID, MESSAGE_RECEIVED_AT,
    MESSAGE_SENDER_ID, MESSAGE_SENT_AT, MESSAGE_SEQ, MESSAGE_STATE, PAIRING_EXPIRES_AT,
    PAIRING_NODE_ID, PAIRING_PIN, PAIRING_PORT, PAIRING_QR, PEER_CONNECTED, PEER_DISPLAY_NAME,
    PEER_FINGERPRINT, PEER_ID, PEER_LAST_SEEN, PEER_POLICY, PEER_TRUSTED_AT,
    STATUS_ACTIVE_SESSIONS, STATUS_OUTBOX_DEPTH, STATUS_PEER_COUNT, STATUS_TRANSPORT_STATE,
};

#[derive(Debug, Error)]
pub enum ParseError {
    #[error("missing field: {0}")]
    MissingField(String),
}

pub type Result<T> = std::result::Result<T, ParseError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PeerPolicy {
    Accept,
    Mute,
    Ignore,
}

impl PeerPolicy {
    pub fn parse(value: &str) -> Self {
        match value {
            "mute" => Self::Mute,
            "ignore" => Self::Ignore,
            _ => Self::Accept,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Accept => "accept",
            Self::Mute => "mute",
            Self::Ignore => "ignore",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageState {
    Queued,
    Sent,
    Delivered,
    Read,
    Failed,
    Expired,
    Unknown,
}

impl MessageState {
    pub fn parse(value: &str) -> Self {
        match value {
            "queued" => Self::Queued,
            "sent" => Self::Sent,
            "delivered" => Self::Delivered,
            "read" => Self::Read,
            "failed" => Self::Failed,
            "expired" => Self::Expired,
            _ => Self::Unknown,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Sent => "sent",
            Self::Delivered => "delivered",
            Self::Read => "read",
            Self::Failed => "failed",
            Self::Expired => "expired",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Peer {
    pub id: String,
    pub fingerprint: String,
    pub display_name: String,
    pub policy: PeerPolicy,
    pub trusted_at: u64,
    pub last_seen: u64,
    pub connected: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    pub node_id: String,
    pub fingerprint: String,
    pub display_name: String,
    pub addresses: String,
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    pub id: String,
    pub conversation_id: String,
    pub peer_id: String,
    pub sender_id: String,
    pub local_sender: bool,
    pub seq: u64,
    pub sent_at: u64,
    pub received_at: u64,
    pub body: String,
    pub state: MessageState,
    pub notify: bool,
}

impl Message {
    pub fn ordering_key(&self) -> (u64, u64, u64) {
        (self.sent_at, self.received_at, self.seq)
    }
}

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

pub fn parse_identity(values: &HashMap<String, OwnedVariant>) -> Result<Identity> {
    Ok(Identity {
        node_id: required_string(values, IDENTITY_NODE_ID)?,
        fingerprint: required_string(values, IDENTITY_FINGERPRINT)?,
        display_name: optional_string(values, IDENTITY_DISPLAY_NAME),
    })
}

pub fn parse_status(values: &HashMap<String, OwnedVariant>) -> Result<Status> {
    Ok(Status {
        transport_state: optional_string(values, STATUS_TRANSPORT_STATE),
        active_sessions: optional_u64(values, STATUS_ACTIVE_SESSIONS) as u32,
        outbox_depth: optional_u64(values, STATUS_OUTBOX_DEPTH) as u32,
        peer_count: optional_u64(values, STATUS_PEER_COUNT) as u32,
    })
}

pub fn parse_peers(values: &[HashMap<String, OwnedVariant>]) -> Result<Vec<Peer>> {
    values.iter().map(parse_peer).collect()
}

pub fn parse_peer(values: &HashMap<String, OwnedVariant>) -> Result<Peer> {
    Ok(Peer {
        id: required_string(values, PEER_ID)?,
        fingerprint: required_string(values, PEER_FINGERPRINT)?,
        display_name: optional_string(values, PEER_DISPLAY_NAME),
        policy: PeerPolicy::parse(&optional_string(values, PEER_POLICY)),
        trusted_at: optional_u64(values, PEER_TRUSTED_AT),
        last_seen: optional_u64(values, PEER_LAST_SEEN),
        connected: optional_bool(values, PEER_CONNECTED),
    })
}

pub fn parse_candidate(values: &HashMap<String, OwnedVariant>) -> Result<Candidate> {
    Ok(Candidate {
        node_id: required_string(values, CANDIDATE_NODE_ID)?,
        fingerprint: required_string(values, CANDIDATE_FINGERPRINT)?,
        display_name: optional_string(values, CANDIDATE_DISPLAY_NAME),
        addresses: optional_string(values, CANDIDATE_ADDRESSES),
        source: optional_string(values, CANDIDATE_SOURCE),
    })
}

pub fn parse_message(
    values: &HashMap<String, OwnedVariant>,
    local_node_id: Option<&str>,
) -> Result<Message> {
    let sender_id = required_string(values, MESSAGE_SENDER_ID)?;
    let local_sender = local_node_id.is_some_and(|local| local == sender_id);
    Ok(Message {
        id: required_string(values, MESSAGE_ID)?,
        conversation_id: required_string(values, MESSAGE_CONVERSATION_ID)?,
        peer_id: optional_string(values, MESSAGE_PEER_ID),
        sender_id,
        local_sender,
        seq: optional_u64(values, MESSAGE_SEQ),
        sent_at: optional_u64(values, MESSAGE_SENT_AT),
        received_at: optional_u64(values, MESSAGE_RECEIVED_AT),
        body: optional_string(values, MESSAGE_BODY),
        state: MessageState::parse(&optional_string(values, MESSAGE_STATE)),
        notify: optional_bool(values, MESSAGE_NOTIFY),
    })
}

pub fn parse_pairing(values: &HashMap<String, OwnedVariant>) -> Result<PairingStart> {
    Ok(PairingStart {
        pin: required_string(values, PAIRING_PIN)?,
        qr: required_string(values, PAIRING_QR)?,
        expires_at: optional_u64(values, PAIRING_EXPIRES_AT),
        node_id: optional_string(values, PAIRING_NODE_ID),
        port: optional_u64(values, PAIRING_PORT) as u32,
    })
}

pub fn settings_map(values: &HashMap<String, OwnedVariant>) -> BTreeMap<String, String> {
    values
        .iter()
        .map(|(key, value)| (key.clone(), display_value(value)))
        .collect()
}

pub fn display_value(value: &OwnedVariant) -> String {
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

pub fn required_string(values: &HashMap<String, OwnedVariant>, key: &str) -> Result<String> {
    values
        .get(key)
        .map(display_value)
        .ok_or_else(|| ParseError::MissingField(key.to_owned()))
}

pub fn optional_string(values: &HashMap<String, OwnedVariant>, key: &str) -> String {
    values.get(key).map(display_value).unwrap_or_default()
}

pub fn optional_u64(values: &HashMap<String, OwnedVariant>, key: &str) -> u64 {
    values
        .get(key)
        .and_then(|value| u64::try_from(value).ok())
        .unwrap_or_default()
}

pub fn optional_bool(values: &HashMap<String, OwnedVariant>, key: &str) -> bool {
    values
        .get(key)
        .and_then(|value| bool::try_from(value).ok())
        .unwrap_or_default()
}
