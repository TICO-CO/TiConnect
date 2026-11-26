use std::collections::{BTreeMap, HashMap};

use serde::{Deserialize, Serialize};
use thiserror::Error;
use zbus::OwnedVariant;

use super::{
    AUDIO_DEVICE_ID, AUDIO_DEVICE_KIND, AUDIO_DEVICE_NAME, CALL_BYTES_SENT, CALL_CONNECTED_AT,
    CALL_DIRECTION, CALL_DURATION_SECS, CALL_ENDED_AT, CALL_FRAMES_RECEIVED, CALL_FRAMES_SENT,
    CALL_ID, CALL_INPUT_MUTED, CALL_OUTPUT_LEVEL, CALL_OUTPUT_MUTED, CALL_PACKETS_CONCEALED,
    CALL_PACKETS_LOST, CALL_PLAYBACK_UNDERRUNS, CALL_REASON, CALL_STARTED_AT, CALL_STATE,
    CALL_USER_ID, CANDIDATE_ADDRESSES, CANDIDATE_AVATAR_HASH, CANDIDATE_DISPLAY_NAME,
    CANDIDATE_FINGERPRINT, CANDIDATE_NODE_ID, CANDIDATE_SOURCE, CONVERSATION_ID,
    CONVERSATION_LAST_BODY, CONVERSATION_LAST_DELETED, CONVERSATION_LAST_MESSAGE_ID,
    CONVERSATION_LAST_SENDER_ID, CONVERSATION_LAST_SENT_AT, CONVERSATION_UNREAD_COUNT,
    CONVERSATION_USER_ID, IDENTITY_AVATAR_HASH, IDENTITY_CREATED_AT, IDENTITY_DISPLAY_NAME,
    IDENTITY_FINGERPRINT, IDENTITY_NODE_ID, MESSAGE_ATTACHMENT_HASH, MESSAGE_ATTACHMENT_MIME,
    MESSAGE_ATTACHMENT_NAME, MESSAGE_ATTACHMENT_PATH, MESSAGE_ATTACHMENT_SIZE, MESSAGE_BODY,
    MESSAGE_CONVERSATION_ID, MESSAGE_DELETED_AT, MESSAGE_EDITED_AT, MESSAGE_ID, MESSAGE_NOTIFY,
    MESSAGE_RECEIVED_AT, MESSAGE_REPLY_TO, MESSAGE_SENDER_ID, MESSAGE_SENT_AT, MESSAGE_SEQ,
    MESSAGE_STATE, MESSAGE_USER_ID, NETWORK_ACTIVE_SESSIONS, NETWORK_BEACON_SEND_ERRORS,
    NETWORK_BEACONS_RECEIVED, NETWORK_BEACONS_SENT, NETWORK_BROADCAST_ACTIVE,
    NETWORK_CANDIDATE_COUNT, NETWORK_CONFIDENCE, NETWORK_DROPPED_PACKETS, NETWORK_HINT,
    NETWORK_INTERFACE_COUNT, NETWORK_INTERFACES, NETWORK_LAST_BEACON_RECEIVED_AT,
    NETWORK_LAST_BEACON_SENT_AT, NETWORK_LAST_CANDIDATE_AT, NETWORK_LAST_FAILURE_AT,
    NETWORK_LAST_SESSION_AT, NETWORK_MDNS_ACTIVE, NETWORK_STATE, NETWORK_TRANSPORT_BOUND,
    NETWORK_TRANSPORT_FAILURES, NETWORK_TRANSPORT_PORT, REMOTE_EXPIRES_AT, REMOTE_FINGERPRINT,
    REMOTE_INDICATOR, REMOTE_INPUT_ALLOWED, REMOTE_ROLE, REMOTE_SESSION_ID, REMOTE_STATE,
    REMOTE_USER_ID, STATUS_ACTIVE_SESSIONS, STATUS_OUTBOX_DEPTH, STATUS_TRANSPORT_STATE,
    STATUS_USER_COUNT, STORAGE_ATTACHMENT_BYTES, STORAGE_ATTACHMENT_COUNT, STORAGE_DATABASE_BYTES,
    STORAGE_MESSAGE_COUNT, TRANSFER_BYTES, TRANSFER_HASH, TRANSFER_NAME, TRANSFER_PHASE,
    TRANSFER_REASON, TRANSFER_SIZE, TRANSFER_USER_ID, USER_ADDRESSES, USER_APP_VERSION,
    USER_AVATAR_HASH, USER_CONNECTED, USER_DESKTOP, USER_DISPLAY_NAME, USER_DISPLAY_SERVER,
    USER_FINGERPRINT, USER_ID, USER_LAST_SEEN, USER_MAC, USER_OS, USER_POLICY, USER_PORT,
    USER_SOURCE, USER_TRUSTED_AT, WATCH_PARTY_ACTOR, WATCH_PARTY_HOST_ID, WATCH_PARTY_ID,
    WATCH_PARTY_INVITED, WATCH_PARTY_PARTICIPANTS, WATCH_PARTY_PLAYING, WATCH_PARTY_POSITION_MS,
    WATCH_PARTY_REVISION, WATCH_PARTY_ROLE, WATCH_PARTY_STATE, WATCH_PARTY_UPDATED_AT,
    WATCH_PARTY_VIDEO_ID,
};

#[derive(Debug, Error)]
pub enum ParseError {
    #[error("missing field: {0}")]
    MissingField(String),

    #[error("field {field} has invalid type: expected {expected}")]
    InvalidType {
        field: String,
        expected: &'static str,
    },

    #[error("field {field} has invalid value: {value}")]
    InvalidValue { field: String, value: String },

    #[error("field {field} value is out of range")]
    OutOfRange { field: String },
}

pub type Result<T> = std::result::Result<T, ParseError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UserPolicy {
    Accept,
    Mute,
    Ignore,
}

impl UserPolicy {
    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "accept" => Ok(Self::Accept),
            "mute" => Ok(Self::Mute),
            "ignore" => Ok(Self::Ignore),
            other => Err(ParseError::InvalidValue {
                field: "policy".to_owned(),
                value: other.to_owned(),
            }),
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
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
    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "queued" => Ok(Self::Queued),
            "sent" => Ok(Self::Sent),
            "delivered" => Ok(Self::Delivered),
            "read" => Ok(Self::Read),
            "failed" => Ok(Self::Failed),
            "expired" => Ok(Self::Expired),
            "unknown" => Ok(Self::Unknown),
            other => Err(ParseError::InvalidValue {
                field: "message state".to_owned(),
                value: other.to_owned(),
            }),
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct User {
    pub id: String,
    pub fingerprint: String,
    pub display_name: String,
    pub avatar_hash: String,
    pub policy: UserPolicy,
    pub trusted_at: u64,
    pub last_seen: u64,
    pub connected: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WatchParty {
    pub party_id: String,
    pub host_id: String,
    pub video_id: String,
    pub role: String,
    pub state: String,
    pub participants: Vec<String>,
    pub invited: Vec<String>,
    pub position_ms: u64,
    pub playing: bool,
    pub revision: u64,
    pub actor: String,
    pub updated_at: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserDetails {
    pub user: User,
    pub addresses: String,
    pub port: u16,
    pub source: String,
    pub mac: String,
    pub os: String,
    pub desktop: String,
    pub display_server: String,
    pub app_version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Candidate {
    pub node_id: String,
    pub fingerprint: String,
    pub display_name: String,
    pub avatar_hash: String,
    pub addresses: String,
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConversationSummary {
    pub conversation_id: String,
    pub user_id: String,
    pub last_message_id: String,
    pub last_sender_id: String,
    pub last_body: String,
    pub last_deleted: bool,
    pub last_sent_at: u64,
    pub unread_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Message {
    pub id: String,
    pub conversation_id: String,
    pub user_id: String,
    pub sender_id: String,
    pub local_sender: bool,
    pub seq: u64,
    pub sent_at: u64,
    pub received_at: u64,
    pub body: String,
    pub reply_to: String,
    pub attachment_hash: String,
    pub attachment_name: String,
    pub attachment_size: u64,
    pub attachment_mime: String,
    pub attachment_path: String,
    pub state: MessageState,
    pub edited_at: u64,
    pub deleted_at: u64,
    pub notify: bool,
}

impl Message {
    pub fn ordering_key(&self) -> (u64, u64, u64) {
        (self.sent_at, self.received_at, self.seq)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CallEndReason {
    Declined,
    Hangup,
    Timeout,
    Error,
    Disconnected,
    Unknown,
}

impl CallEndReason {
    pub fn parse(value: &str) -> Self {
        match value {
            "declined" => Self::Declined,
            "hangup" => Self::Hangup,
            "timeout" => Self::Timeout,
            "error" => Self::Error,
            "disconnected" => Self::Disconnected,
            _ => Self::Unknown,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Declined => "declined",
            Self::Hangup => "hangup",
            Self::Timeout => "timeout",
            Self::Error => "error",
            Self::Disconnected => "disconnected",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RemoteEndReason {
    #[serde(rename = "denied")]
    Denied,
    #[serde(rename = "expired")]
    Expired,
    #[serde(rename = "start-failed")]
    StartFailed,
    #[serde(rename = "unreachable")]
    Unreachable,
    #[serde(rename = "kill-switch")]
    KillSwitch,
    #[serde(rename = "disconnected")]
    Disconnected,
    #[serde(rename = "hangup")]
    Hangup,
    #[serde(rename = "left")]
    Left,
    #[serde(rename = "ended")]
    Ended,
    #[serde(rename = "unknown")]
    Unknown,
}

impl RemoteEndReason {
    pub fn parse(value: &str) -> Self {
        match value {
            "denied" => Self::Denied,
            "expired" => Self::Expired,
            "start-failed" => Self::StartFailed,
            "unreachable" => Self::Unreachable,
            "kill-switch" => Self::KillSwitch,
            "disconnected" => Self::Disconnected,
            "hangup" => Self::Hangup,
            "left" => Self::Left,
            "ended" => Self::Ended,
            _ => Self::Unknown,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Denied => "denied",
            Self::Expired => "expired",
            Self::StartFailed => "start-failed",
            Self::Unreachable => "unreachable",
            Self::KillSwitch => "kill-switch",
            Self::Disconnected => "disconnected",
            Self::Hangup => "hangup",
            Self::Left => "left",
            Self::Ended => "ended",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WatchPartyEndReason {
    #[serde(rename = "declined")]
    Declined,
    #[serde(rename = "left")]
    Left,
    #[serde(rename = "host ended")]
    HostEnded,
    #[serde(rename = "host disconnected")]
    HostDisconnected,
    #[serde(rename = "unknown")]
    Unknown,
}

impl WatchPartyEndReason {
    pub fn parse(value: &str) -> Self {
        match value {
            "declined" => Self::Declined,
            "left" => Self::Left,
            "host ended" => Self::HostEnded,
            "host disconnected" => Self::HostDisconnected,
            _ => Self::Unknown,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Declined => "declined",
            Self::Left => "left",
            Self::HostEnded => "host ended",
            Self::HostDisconnected => "host disconnected",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteSession {
    pub session_id: String,
    pub user_id: String,
    pub fingerprint: String,
    pub role: String,
    pub state: String,
    pub indicator: bool,
    pub input_allowed: bool,
    pub expires_at: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Call {
    pub id: String,
    pub user_id: String,
    pub direction: String,
    pub state: String,
    pub started_at: u64,
    pub connected_at: u64,
    pub input_muted: bool,
    pub output_muted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CallLog {
    pub id: String,
    pub user_id: String,
    pub direction: String,
    pub started_at: u64,
    pub connected_at: u64,
    pub ended_at: u64,
    pub duration_secs: u64,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CallMetrics {
    pub frames_sent: u64,
    pub frames_received: u64,
    pub packets_lost: u64,
    pub packets_concealed: u64,
    pub playback_underruns: u64,
    pub bytes_sent: u64,
    pub output_level: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AudioDevice {
    pub id: String,
    pub name: String,
    pub kind: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Transfer {
    pub hash: String,
    pub user_id: String,
    pub name: String,
    pub size: u64,
    pub bytes: u64,
    pub phase: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Identity {
    pub node_id: String,
    pub fingerprint: String,
    pub display_name: String,
    pub avatar_hash: String,
    pub created_at: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Status {
    pub transport_state: String,
    pub active_sessions: u32,
    pub outbox_depth: u32,
    pub user_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NetworkStatus {
    pub state: String,
    pub confidence: String,
    pub hint: String,
    pub interfaces: String,
    pub interface_count: u32,
    pub transport_port: u16,
    pub transport_bound: bool,
    pub mdns_active: bool,
    pub broadcast_active: bool,
    pub candidate_count: u32,
    pub active_sessions: u32,
    pub beacons_sent: u64,
    pub beacons_received: u64,
    pub beacon_send_errors: u64,
    pub dropped_packets: u64,
    pub last_beacon_sent_at: u64,
    pub last_beacon_received_at: u64,
    pub last_candidate_at: u64,
    pub last_session_at: u64,
    pub last_failure_at: u64,
    pub transport_failures: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct StorageUsage {
    pub message_count: u64,
    pub attachment_count: u64,
    pub attachment_bytes: u64,
    pub database_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DaemonEvent {
    Connected,
    Disconnected(String),
    Users(Vec<User>),
    Candidates(Vec<Candidate>),
    CandidateLost(String),
    MessageReceived(Message),
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
    UserConnected(User),
    UserDisconnected(User),
    PolicyChanged {
        user_id: String,
        policy: UserPolicy,
    },
    SettingsChanged(String),
    SyncCompleted {
        user_id: String,
        count: u32,
    },
    TransferProgress {
        hash: String,
        bytes: u64,
        total: u64,
    },
    TransferDone(String),
    TransferFailed {
        hash: String,
        reason: String,
    },
    IncomingCall(Call),
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
        reason: CallEndReason,
    },
    RemoteSessionRequested(RemoteSession),
    RemoteSessionStarted(String),
    RemoteSessionEnded {
        session_id: String,
        reason: RemoteEndReason,
    },
    RemoteIndicatorChanged {
        session_id: String,
        visible: bool,
    },
    RemoteInputChanged {
        session_id: String,
        allowed: bool,
    },
    WatchPartyRequested(WatchParty),
    WatchPartyChanged(WatchParty),
    WatchPartyEnded {
        party_id: String,
        reason: WatchPartyEndReason,
    },
}

pub fn parse_identity(values: &HashMap<String, OwnedVariant>) -> Result<Identity> {
    Ok(Identity {
        node_id: required_string(values, IDENTITY_NODE_ID)?,
        fingerprint: required_string(values, IDENTITY_FINGERPRINT)?,
        display_name: optional_string(values, IDENTITY_DISPLAY_NAME)?,
        avatar_hash: optional_string(values, IDENTITY_AVATAR_HASH)?,
        created_at: optional_u64(values, IDENTITY_CREATED_AT)?,
    })
}

pub fn parse_status(values: &HashMap<String, OwnedVariant>) -> Result<Status> {
    Ok(Status {
        transport_state: optional_string(values, STATUS_TRANSPORT_STATE)?,
        active_sessions: optional_u32(values, STATUS_ACTIVE_SESSIONS)?,
        outbox_depth: optional_u32(values, STATUS_OUTBOX_DEPTH)?,
        user_count: optional_u32(values, STATUS_USER_COUNT)?,
    })
}

pub fn parse_network_status(values: &HashMap<String, OwnedVariant>) -> Result<NetworkStatus> {
    Ok(NetworkStatus {
        state: optional_string(values, NETWORK_STATE)?,
        confidence: optional_string(values, NETWORK_CONFIDENCE)?,
        hint: optional_string(values, NETWORK_HINT)?,
        interfaces: optional_string(values, NETWORK_INTERFACES)?,
        interface_count: optional_u32(values, NETWORK_INTERFACE_COUNT)?,
        transport_port: optional_u16(values, NETWORK_TRANSPORT_PORT)?,
        transport_bound: optional_bool(values, NETWORK_TRANSPORT_BOUND)?,
        mdns_active: optional_bool(values, NETWORK_MDNS_ACTIVE)?,
        broadcast_active: optional_bool(values, NETWORK_BROADCAST_ACTIVE)?,
        candidate_count: optional_u32(values, NETWORK_CANDIDATE_COUNT)?,
        active_sessions: optional_u32(values, NETWORK_ACTIVE_SESSIONS)?,
        beacons_sent: optional_u64(values, NETWORK_BEACONS_SENT)?,
        beacons_received: optional_u64(values, NETWORK_BEACONS_RECEIVED)?,
        beacon_send_errors: optional_u64(values, NETWORK_BEACON_SEND_ERRORS)?,
        dropped_packets: optional_u64(values, NETWORK_DROPPED_PACKETS)?,
        last_beacon_sent_at: optional_u64(values, NETWORK_LAST_BEACON_SENT_AT)?,
        last_beacon_received_at: optional_u64(values, NETWORK_LAST_BEACON_RECEIVED_AT)?,
        last_candidate_at: optional_u64(values, NETWORK_LAST_CANDIDATE_AT)?,
        last_session_at: optional_u64(values, NETWORK_LAST_SESSION_AT)?,
        last_failure_at: optional_u64(values, NETWORK_LAST_FAILURE_AT)?,
        transport_failures: optional_u64(values, NETWORK_TRANSPORT_FAILURES)?,
    })
}

pub fn parse_storage_usage(values: &HashMap<String, OwnedVariant>) -> Result<StorageUsage> {
    Ok(StorageUsage {
        message_count: optional_u64(values, STORAGE_MESSAGE_COUNT)?,
        attachment_count: optional_u64(values, STORAGE_ATTACHMENT_COUNT)?,
        attachment_bytes: optional_u64(values, STORAGE_ATTACHMENT_BYTES)?,
        database_bytes: optional_u64(values, STORAGE_DATABASE_BYTES)?,
    })
}

pub fn parse_users(values: &[HashMap<String, OwnedVariant>]) -> Result<Vec<User>> {
    values.iter().map(parse_user).collect()
}

pub fn parse_user(values: &HashMap<String, OwnedVariant>) -> Result<User> {
    Ok(User {
        id: required_string(values, USER_ID)?,
        fingerprint: required_string(values, USER_FINGERPRINT)?,
        display_name: optional_string(values, USER_DISPLAY_NAME)?,
        avatar_hash: optional_string(values, USER_AVATAR_HASH)?,
        policy: if values.contains_key(USER_POLICY) {
            UserPolicy::parse(&optional_string(values, USER_POLICY)?)?
        } else {
            UserPolicy::Accept
        },
        trusted_at: optional_u64(values, USER_TRUSTED_AT)?,
        last_seen: optional_u64(values, USER_LAST_SEEN)?,
        connected: optional_bool(values, USER_CONNECTED)?,
    })
}

pub fn parse_watch_party(values: &HashMap<String, OwnedVariant>) -> Result<WatchParty> {
    Ok(WatchParty {
        party_id: required_string(values, WATCH_PARTY_ID)?,
        host_id: optional_string(values, WATCH_PARTY_HOST_ID)?,
        video_id: optional_string(values, WATCH_PARTY_VIDEO_ID)?,
        role: optional_string(values, WATCH_PARTY_ROLE)?,
        state: optional_string(values, WATCH_PARTY_STATE)?,
        participants: optional_list(values, WATCH_PARTY_PARTICIPANTS),
        invited: optional_list(values, WATCH_PARTY_INVITED),
        position_ms: optional_u64(values, WATCH_PARTY_POSITION_MS)?,
        playing: optional_bool(values, WATCH_PARTY_PLAYING)?,
        revision: optional_u64(values, WATCH_PARTY_REVISION)?,
        actor: optional_string(values, WATCH_PARTY_ACTOR)?,
        updated_at: optional_u64(values, WATCH_PARTY_UPDATED_AT)?,
    })
}

fn optional_list(values: &HashMap<String, OwnedVariant>, key: &str) -> Vec<String> {
    let Some(value) = values.get(key) else {
        return Vec::new();
    };
    let Ok(value) = <&str>::try_from(value) else {
        return Vec::new();
    };
    value
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(str::to_owned)
        .collect()
}

pub fn parse_user_details(values: &HashMap<String, OwnedVariant>) -> Result<UserDetails> {
    Ok(UserDetails {
        user: parse_user(values)?,
        addresses: optional_string(values, USER_ADDRESSES)?,
        port: optional_u16(values, USER_PORT)?,
        source: optional_string(values, USER_SOURCE)?,
        mac: optional_string(values, USER_MAC)?,
        os: optional_string(values, USER_OS)?,
        desktop: optional_string(values, USER_DESKTOP)?,
        display_server: optional_string(values, USER_DISPLAY_SERVER)?,
        app_version: optional_string(values, USER_APP_VERSION)?,
    })
}

pub fn parse_candidate(values: &HashMap<String, OwnedVariant>) -> Result<Candidate> {
    Ok(Candidate {
        node_id: required_string(values, CANDIDATE_NODE_ID)?,
        fingerprint: required_string(values, CANDIDATE_FINGERPRINT)?,
        display_name: optional_string(values, CANDIDATE_DISPLAY_NAME)?,
        avatar_hash: optional_string(values, CANDIDATE_AVATAR_HASH)?,
        addresses: optional_string(values, CANDIDATE_ADDRESSES)?,
        source: optional_string(values, CANDIDATE_SOURCE)?,
    })
}

pub fn parse_conversation_summary(
    values: &HashMap<String, OwnedVariant>,
) -> Result<ConversationSummary> {
    Ok(ConversationSummary {
        conversation_id: required_string(values, CONVERSATION_ID)?,
        user_id: required_string(values, CONVERSATION_USER_ID)?,
        last_message_id: optional_string(values, CONVERSATION_LAST_MESSAGE_ID)?,
        last_sender_id: optional_string(values, CONVERSATION_LAST_SENDER_ID)?,
        last_body: optional_string(values, CONVERSATION_LAST_BODY)?,
        last_deleted: optional_bool(values, CONVERSATION_LAST_DELETED)?,
        last_sent_at: optional_u64(values, CONVERSATION_LAST_SENT_AT)?,
        unread_count: optional_u32(values, CONVERSATION_UNREAD_COUNT)?,
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
        user_id: optional_string(values, MESSAGE_USER_ID)?,
        sender_id,
        local_sender,
        seq: optional_u64(values, MESSAGE_SEQ)?,
        sent_at: optional_u64(values, MESSAGE_SENT_AT)?,
        received_at: optional_u64(values, MESSAGE_RECEIVED_AT)?,
        body: optional_string(values, MESSAGE_BODY)?,
        reply_to: optional_string(values, MESSAGE_REPLY_TO)?,
        attachment_hash: optional_string(values, MESSAGE_ATTACHMENT_HASH)?,
        attachment_name: optional_string(values, MESSAGE_ATTACHMENT_NAME)?,
        attachment_size: optional_u64(values, MESSAGE_ATTACHMENT_SIZE)?,
        attachment_mime: optional_string(values, MESSAGE_ATTACHMENT_MIME)?,
        attachment_path: optional_string(values, MESSAGE_ATTACHMENT_PATH)?,
        state: {
            let state = optional_string(values, MESSAGE_STATE)?;
            if state.is_empty() {
                MessageState::Unknown
            } else {
                MessageState::parse(&state)?
            }
        },
        edited_at: optional_u64(values, MESSAGE_EDITED_AT)?,
        deleted_at: optional_u64(values, MESSAGE_DELETED_AT)?,
        notify: optional_bool(values, MESSAGE_NOTIFY)?,
    })
}

pub fn parse_remote_session(values: &HashMap<String, OwnedVariant>) -> Result<RemoteSession> {
    Ok(RemoteSession {
        session_id: required_string(values, REMOTE_SESSION_ID)?,
        user_id: optional_string(values, REMOTE_USER_ID)?,
        fingerprint: optional_string(values, REMOTE_FINGERPRINT)?,
        role: optional_string(values, REMOTE_ROLE)?,
        state: optional_string(values, REMOTE_STATE)?,
        indicator: optional_bool(values, REMOTE_INDICATOR)?,
        input_allowed: optional_bool(values, REMOTE_INPUT_ALLOWED)?,
        expires_at: optional_u64(values, REMOTE_EXPIRES_AT)?,
    })
}

pub fn parse_call(values: &HashMap<String, OwnedVariant>) -> Result<Call> {
    Ok(Call {
        id: required_string(values, CALL_ID)?,
        user_id: optional_string(values, CALL_USER_ID)?,
        direction: optional_string(values, CALL_DIRECTION)?,
        state: optional_string(values, CALL_STATE)?,
        started_at: optional_u64(values, CALL_STARTED_AT)?,
        connected_at: optional_u64(values, CALL_CONNECTED_AT)?,
        input_muted: optional_bool(values, CALL_INPUT_MUTED)?,
        output_muted: optional_bool(values, CALL_OUTPUT_MUTED)?,
    })
}

pub fn parse_call_log(values: &HashMap<String, OwnedVariant>) -> Result<CallLog> {
    Ok(CallLog {
        id: required_string(values, CALL_ID)?,
        user_id: optional_string(values, CALL_USER_ID)?,
        direction: optional_string(values, CALL_DIRECTION)?,
        started_at: optional_u64(values, CALL_STARTED_AT)?,
        connected_at: optional_u64(values, CALL_CONNECTED_AT)?,
        ended_at: optional_u64(values, CALL_ENDED_AT)?,
        duration_secs: optional_u64(values, CALL_DURATION_SECS)?,
        reason: optional_string(values, CALL_REASON)?,
    })
}

pub fn parse_call_metrics(values: &HashMap<String, OwnedVariant>) -> Result<CallMetrics> {
    Ok(CallMetrics {
        frames_sent: optional_u64(values, CALL_FRAMES_SENT)?,
        frames_received: optional_u64(values, CALL_FRAMES_RECEIVED)?,
        packets_lost: optional_u64(values, CALL_PACKETS_LOST)?,
        packets_concealed: optional_u64(values, CALL_PACKETS_CONCEALED)?,
        playback_underruns: optional_u64(values, CALL_PLAYBACK_UNDERRUNS)?,
        bytes_sent: optional_u64(values, CALL_BYTES_SENT)?,
        output_level: optional_u16(values, CALL_OUTPUT_LEVEL)?,
    })
}

pub fn parse_audio_device(values: &HashMap<String, OwnedVariant>) -> Result<AudioDevice> {
    Ok(AudioDevice {
        id: required_string(values, AUDIO_DEVICE_ID)?,
        name: optional_string(values, AUDIO_DEVICE_NAME)?,
        kind: optional_string(values, AUDIO_DEVICE_KIND)?,
    })
}

pub fn parse_transfer(values: &HashMap<String, OwnedVariant>) -> Result<Transfer> {
    Ok(Transfer {
        hash: required_string(values, TRANSFER_HASH)?,
        user_id: optional_string(values, TRANSFER_USER_ID)?,
        name: optional_string(values, TRANSFER_NAME)?,
        size: optional_u64(values, TRANSFER_SIZE)?,
        bytes: optional_u64(values, TRANSFER_BYTES)?,
        phase: optional_string(values, TRANSFER_PHASE)?,
        reason: optional_string(values, TRANSFER_REASON)?,
    })
}

pub fn settings_map(values: &HashMap<String, OwnedVariant>) -> Result<BTreeMap<String, String>> {
    values
        .iter()
        .map(|(key, value)| Ok((key.clone(), display_value(value, key)?)))
        .collect()
}

pub fn display_value(value: &OwnedVariant, field: &str) -> Result<String> {
    if let Ok(value) = u64::try_from(value) {
        Ok(value.to_string())
    } else if let Ok(value) = u32::try_from(value) {
        Ok(value.to_string())
    } else if let Ok(value) = bool::try_from(value) {
        Ok(value.to_string())
    } else if let Ok(value) = <&str>::try_from(value) {
        Ok(value.to_owned())
    } else {
        Err(ParseError::InvalidType {
            field: field.to_owned(),
            expected: "string, boolean, or unsigned integer",
        })
    }
}

pub fn required_string(values: &HashMap<String, OwnedVariant>, key: &str) -> Result<String> {
    let value = values
        .get(key)
        .ok_or_else(|| ParseError::MissingField(key.to_owned()))?;
    <&str>::try_from(value)
        .map(str::to_owned)
        .map_err(|_| ParseError::InvalidType {
            field: key.to_owned(),
            expected: "string",
        })
}

pub fn optional_string(values: &HashMap<String, OwnedVariant>, key: &str) -> Result<String> {
    let Some(value) = values.get(key) else {
        return Ok(String::new());
    };
    <&str>::try_from(value)
        .map(str::to_owned)
        .map_err(|_| ParseError::InvalidType {
            field: key.to_owned(),
            expected: "string",
        })
}

pub fn optional_u64(values: &HashMap<String, OwnedVariant>, key: &str) -> Result<u64> {
    let Some(value) = values.get(key) else {
        return Ok(0);
    };
    variant_u64(value, key)
}

pub fn optional_u32(values: &HashMap<String, OwnedVariant>, key: &str) -> Result<u32> {
    let value = optional_u64(values, key)?;
    u32::try_from(value).map_err(|_| ParseError::OutOfRange {
        field: key.to_owned(),
    })
}

pub fn optional_u16(values: &HashMap<String, OwnedVariant>, key: &str) -> Result<u16> {
    let value = optional_u64(values, key)?;
    u16::try_from(value).map_err(|_| ParseError::OutOfRange {
        field: key.to_owned(),
    })
}

pub fn optional_bool(values: &HashMap<String, OwnedVariant>, key: &str) -> Result<bool> {
    let Some(value) = values.get(key) else {
        return Ok(false);
    };
    bool::try_from(value).map_err(|_| ParseError::InvalidType {
        field: key.to_owned(),
        expected: "boolean",
    })
}

fn variant_u64(value: &OwnedVariant, key: &str) -> Result<u64> {
    if let Ok(value) = u64::try_from(value) {
        Ok(value)
    } else if let Ok(value) = u32::try_from(value) {
        Ok(u64::from(value))
    } else if let Ok(value) = u16::try_from(value) {
        Ok(u64::from(value))
    } else if let Ok(value) = u8::try_from(value) {
        Ok(u64::from(value))
    } else if let Ok(value) = i64::try_from(value) {
        u64::try_from(value).map_err(|_| ParseError::OutOfRange {
            field: key.to_owned(),
        })
    } else {
        Err(ParseError::InvalidType {
            field: key.to_owned(),
            expected: "unsigned integer",
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn policy_and_state_use_wire_names() {
        assert_eq!(
            serde_json::to_string(&UserPolicy::Accept).unwrap(),
            "\"accept\""
        );
        assert_eq!(
            serde_json::to_string(&MessageState::Delivered).unwrap(),
            "\"delivered\""
        );
    }

    #[test]
    fn signed_integer_timestamps_are_accepted() {
        let mut values = HashMap::new();
        values.insert(
            CALL_ID.to_owned(),
            OwnedVariant::from(zbus::Str::from("call-1")),
        );
        values.insert(CALL_STARTED_AT.to_owned(), OwnedVariant::from(123_i64));
        values.insert(CALL_CONNECTED_AT.to_owned(), OwnedVariant::from(456_i64));
        let call = parse_call(&values).expect("call");
        assert_eq!(call.started_at, 123);
        assert_eq!(call.connected_at, 456);
    }

    #[test]
    fn call_metrics_parse_levels_and_counters() {
        let values = HashMap::from([
            (CALL_FRAMES_SENT.to_owned(), OwnedVariant::from(12_u64)),
            (CALL_FRAMES_RECEIVED.to_owned(), OwnedVariant::from(34_u64)),
            (CALL_PACKETS_LOST.to_owned(), OwnedVariant::from(2_u64)),
            (CALL_PACKETS_CONCEALED.to_owned(), OwnedVariant::from(3_u64)),
            (
                CALL_PLAYBACK_UNDERRUNS.to_owned(),
                OwnedVariant::from(4_u64),
            ),
            (CALL_BYTES_SENT.to_owned(), OwnedVariant::from(960_u64)),
            (CALL_OUTPUT_LEVEL.to_owned(), OwnedVariant::from(512_u16)),
        ]);
        let metrics = parse_call_metrics(&values).expect("call metrics");
        assert_eq!(metrics.frames_sent, 12);
        assert_eq!(metrics.frames_received, 34);
        assert_eq!(metrics.packets_lost, 2);
        assert_eq!(metrics.packets_concealed, 3);
        assert_eq!(metrics.playback_underruns, 4);
        assert_eq!(metrics.bytes_sent, 960);
        assert_eq!(metrics.output_level, 512);
    }

    #[test]
    fn network_status_parses_typed_values() {
        let values = HashMap::from([
            (
                NETWORK_STATE.to_owned(),
                OwnedVariant::from(zbus::Str::from("reachable")),
            ),
            (
                NETWORK_CONFIDENCE.to_owned(),
                OwnedVariant::from(zbus::Str::from("confirmed")),
            ),
            (
                NETWORK_TRANSPORT_PORT.to_owned(),
                OwnedVariant::from(45871_u32),
            ),
            (NETWORK_TRANSPORT_BOUND.to_owned(), OwnedVariant::from(true)),
            (
                NETWORK_CANDIDATE_COUNT.to_owned(),
                OwnedVariant::from(2_u32),
            ),
            (
                NETWORK_ACTIVE_SESSIONS.to_owned(),
                OwnedVariant::from(1_u32),
            ),
        ]);
        let status = parse_network_status(&values).expect("network status");
        assert_eq!(status.state, "reachable");
        assert_eq!(status.confidence, "confirmed");
        assert_eq!(status.transport_port, 45871);
        assert!(status.transport_bound);
        assert_eq!(status.candidate_count, 2);
        assert_eq!(status.active_sessions, 1);
    }

    #[test]
    fn message_serializes_for_the_bridge() {
        let message = Message {
            id: "m1".to_owned(),
            conversation_id: "c1".to_owned(),
            user_id: "p1".to_owned(),
            sender_id: "p1".to_owned(),
            local_sender: false,
            seq: 7,
            sent_at: 10,
            received_at: 11,
            body: "hi".to_owned(),
            reply_to: String::new(),
            attachment_hash: String::new(),
            attachment_name: String::new(),
            attachment_size: 0,
            attachment_mime: String::new(),
            attachment_path: String::new(),
            state: MessageState::Sent,
            edited_at: 0,
            deleted_at: 0,
            notify: true,
        };
        let value = serde_json::to_value(&message).unwrap();
        assert_eq!(value["id"], "m1");
        assert_eq!(value["state"], "sent");
        assert_eq!(value["local_sender"], false);
    }

    #[test]
    fn end_reasons_keep_wire_strings() {
        for reason in [
            CallEndReason::Declined,
            CallEndReason::Hangup,
            CallEndReason::Timeout,
            CallEndReason::Error,
            CallEndReason::Disconnected,
        ] {
            assert_eq!(CallEndReason::parse(reason.as_str()), reason);
        }
        for reason in [
            RemoteEndReason::Denied,
            RemoteEndReason::Expired,
            RemoteEndReason::StartFailed,
            RemoteEndReason::Unreachable,
            RemoteEndReason::KillSwitch,
            RemoteEndReason::Disconnected,
            RemoteEndReason::Hangup,
            RemoteEndReason::Left,
            RemoteEndReason::Ended,
        ] {
            assert_eq!(RemoteEndReason::parse(reason.as_str()), reason);
        }
        for reason in [
            WatchPartyEndReason::Declined,
            WatchPartyEndReason::Left,
            WatchPartyEndReason::HostEnded,
            WatchPartyEndReason::HostDisconnected,
        ] {
            assert_eq!(WatchPartyEndReason::parse(reason.as_str()), reason);
        }
        assert_eq!(
            RemoteEndReason::parse("future-reason"),
            RemoteEndReason::Unknown
        );
        assert_eq!(
            WatchPartyEndReason::parse("future-reason"),
            WatchPartyEndReason::Unknown
        );
    }

    #[test]
    fn daemon_events_round_trip() {
        let event = DaemonEvent::MessageStateChanged {
            message_id: "m1".to_owned(),
            state: MessageState::Read,
        };
        let json = serde_json::to_string(&event).unwrap();
        assert_eq!(
            json,
            r#"{"message_state_changed":{"message_id":"m1","state":"read"}}"#
        );
    }
}
