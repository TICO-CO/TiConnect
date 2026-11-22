pub const DAEMON_BUS_NAME: &str = "org.ticonnect.Daemon";
pub const DAEMON_PATH: &str = "/org/ticonnect/Daemon";
pub const DAEMON_INTERFACE: &str = "org.ticonnect.Daemon";
pub const UI_BUS_NAME: &str = "org.ticonnect.UI";
pub const UI_PATH: &str = "/org/ticonnect/UI";
pub const UI_INTERFACE: &str = "org.ticonnect.UI";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const IPC_BUS_ADDRESS_ENV: &str = "TICONNECT_IPC_BUS_ADDRESS";

pub fn ipc_bus_address() -> Option<String> {
    ipc_bus_address_from(std::env::var_os(IPC_BUS_ADDRESS_ENV))
}

fn ipc_bus_address_from(value: Option<std::ffi::OsString>) -> Option<String> {
    value
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string_lossy().into_owned())
}

pub fn connection_builder() -> zbus::connection::Builder<'static> {
    match ipc_bus_address() {
        Some(address) => zbus::connection::Builder::address(address.as_str()),
        None => zbus::connection::Builder::session(),
    }
}

pub async fn connect() -> zbus::Result<zbus::Connection> {
    connection_builder().build().await
}

pub const STATUS_UPTIME_SECONDS: &str = "uptime_seconds";
pub const STATUS_USER_COUNT: &str = "user_count";
pub const STATUS_OUTBOX_DEPTH: &str = "outbox_depth";
pub const STATUS_TRANSPORT_STATE: &str = "transport_state";
pub const STATUS_ACTIVE_SESSIONS: &str = "active_sessions";
pub const TRANSPORT_OFFLINE: &str = "offline";
pub const TRANSPORT_IDLE: &str = "idle";
pub const TRANSPORT_ONLINE: &str = "online";

pub const IDENTITY_NODE_ID: &str = "node_id";
pub const IDENTITY_FINGERPRINT: &str = "fingerprint";
pub const IDENTITY_DISPLAY_NAME: &str = "display_name";
pub const IDENTITY_AVATAR_HASH: &str = "avatar_hash";
pub const IDENTITY_CREATED_AT: &str = "created_at";

pub const USER_ID: &str = "id";
pub const USER_FINGERPRINT: &str = "fingerprint";
pub const USER_DISPLAY_NAME: &str = "display_name";
pub const USER_POLICY: &str = "policy";
pub const USER_TRUSTED_AT: &str = "trusted_at";
pub const USER_LAST_SEEN: &str = "last_seen";
pub const USER_ADDRESS: &str = "address";
pub const USER_CONNECTED: &str = "connected";
pub const USER_AVATAR_HASH: &str = "avatar_hash";
pub const USER_ADDRESSES: &str = "addresses";
pub const USER_PORT: &str = "port";
pub const USER_SOURCE: &str = "source";
pub const USER_MAC: &str = "mac";
pub const USER_OS: &str = "os";
pub const USER_DESKTOP: &str = "desktop";
pub const USER_DISPLAY_SERVER: &str = "display_server";
pub const USER_APP_VERSION: &str = "app_version";
pub const BLOCKED_AT: &str = "blocked_at";

pub const STORAGE_MESSAGE_COUNT: &str = "message_count";
pub const STORAGE_ATTACHMENT_COUNT: &str = "attachment_count";
pub const STORAGE_ATTACHMENT_BYTES: &str = "attachment_bytes";
pub const STORAGE_DATABASE_BYTES: &str = "database_bytes";

pub const CANDIDATE_NODE_ID: &str = "node_id";
pub const CANDIDATE_FINGERPRINT: &str = "fingerprint";
pub const CANDIDATE_DISPLAY_NAME: &str = "display_name";
pub const CANDIDATE_AVATAR_HASH: &str = "avatar_hash";
pub const CANDIDATE_ADDRESSES: &str = "addresses";
pub const CANDIDATE_PORT: &str = "port";
pub const CANDIDATE_PROTOCOL_VERSION: &str = "protocol_version";
pub const CANDIDATE_SOURCE: &str = "source";
pub const CANDIDATE_LAST_SEEN: &str = "last_seen";

pub const DISCOVERY_ENABLED: &str = "enabled";
pub const DISCOVERY_MDNS_ENABLED: &str = "mdns_enabled";
pub const DISCOVERY_BROADCAST_ENABLED: &str = "broadcast_enabled";
pub const DISCOVERY_MDNS_ACTIVE: &str = "mdns_active";
pub const DISCOVERY_BROADCAST_ACTIVE: &str = "broadcast_active";
pub const DISCOVERY_CANDIDATE_COUNT: &str = "candidate_count";
pub const DISCOVERY_DROPPED_PACKETS: &str = "dropped_packets";

pub const MESSAGE_ID: &str = "message_id";
pub const MESSAGE_CONVERSATION_ID: &str = "conversation_id";
pub const MESSAGE_USER_ID: &str = "user_id";
pub const MESSAGE_SENDER_ID: &str = "sender_id";
pub const MESSAGE_SEQ: &str = "seq";
pub const MESSAGE_SENT_AT: &str = "sent_at";
pub const MESSAGE_RECEIVED_AT: &str = "received_at";
pub const MESSAGE_KIND: &str = "kind";
pub const MESSAGE_TARGET_ID: &str = "target_id";
pub const MESSAGE_BODY: &str = "body";
pub const MESSAGE_ATTACHMENT_HASH: &str = "attachment_hash";
pub const MESSAGE_ATTACHMENT_NAME: &str = "attachment_name";
pub const MESSAGE_ATTACHMENT_SIZE: &str = "attachment_size";
pub const MESSAGE_ATTACHMENT_MIME: &str = "attachment_mime";
pub const MESSAGE_ATTACHMENT_PATH: &str = "attachment_path";
pub const REMOTE_SESSION_ID: &str = "session_id";
pub const REMOTE_USER_ID: &str = "user_id";
pub const REMOTE_FINGERPRINT: &str = "fingerprint";
pub const REMOTE_ROLE: &str = "role";
pub const REMOTE_STATE: &str = "state";
pub const REMOTE_INDICATOR: &str = "indicator";
pub const REMOTE_EXPIRES_AT: &str = "expires_at";
pub const CALL_ID: &str = "call_id";
pub const CALL_USER_ID: &str = "user_id";
pub const CALL_DIRECTION: &str = "direction";
pub const CALL_STATE: &str = "state";
pub const CALL_STARTED_AT: &str = "started_at";
pub const CALL_CONNECTED_AT: &str = "connected_at";
pub const CALL_ENDED_AT: &str = "ended_at";
pub const CALL_DURATION_SECS: &str = "duration_secs";
pub const CALL_REASON: &str = "reason";
pub const CALL_INPUT_MUTED: &str = "input_muted";
pub const CALL_OUTPUT_MUTED: &str = "output_muted";
pub const CALL_FRAMES_SENT: &str = "frames_sent";
pub const CALL_FRAMES_RECEIVED: &str = "frames_received";
pub const CALL_PACKETS_LOST: &str = "packets_lost";
pub const CALL_BYTES_SENT: &str = "bytes_sent";
pub const AUDIO_DEVICE_ID: &str = "id";
pub const AUDIO_DEVICE_NAME: &str = "name";
pub const AUDIO_DEVICE_KIND: &str = "kind";
pub const WATCH_PARTY_ID: &str = "party_id";
pub const WATCH_PARTY_HOST_ID: &str = "host_id";
pub const WATCH_PARTY_VIDEO_ID: &str = "video_id";
pub const WATCH_PARTY_ROLE: &str = "role";
pub const WATCH_PARTY_STATE: &str = "state";
pub const WATCH_PARTY_POSITION_MS: &str = "position_ms";
pub const WATCH_PARTY_PLAYING: &str = "playing";
pub const WATCH_PARTY_REVISION: &str = "revision";
pub const WATCH_PARTY_ACTOR: &str = "actor";
pub const WATCH_PARTY_UPDATED_AT: &str = "updated_at";
pub const WATCH_PARTY_PARTICIPANTS: &str = "participants";
pub const WATCH_PARTY_INVITED: &str = "invited";
pub const TRANSFER_HASH: &str = "hash";
pub const TRANSFER_USER_ID: &str = "user_id";
pub const TRANSFER_NAME: &str = "name";
pub const TRANSFER_SIZE: &str = "size";
pub const TRANSFER_BYTES: &str = "bytes";
pub const TRANSFER_PHASE: &str = "phase";
pub const TRANSFER_REASON: &str = "reason";
pub const MESSAGE_STATE: &str = "state";
pub const MESSAGE_NOTIFY: &str = "notify";
pub const CONVERSATION_ID: &str = "conversation_id";
pub const CONVERSATION_USER_ID: &str = "user_id";
pub const CONVERSATION_LAST_MESSAGE_ID: &str = "last_message_id";
pub const CONVERSATION_LAST_SENDER_ID: &str = "last_sender_id";
pub const CONVERSATION_LAST_BODY: &str = "last_body";
pub const CONVERSATION_LAST_SENT_AT: &str = "last_sent_at";
pub const CONVERSATION_UNREAD_COUNT: &str = "unread_count";

pub mod methods {
    pub const PING: &str = "Ping";
    pub const GET_VERSION: &str = "GetVersion";
    pub const GET_STATUS: &str = "GetStatus";
    pub const GET_IDENTITY: &str = "GetIdentity";
    pub const GET_AVATAR: &str = "GetAvatar";
    pub const SET_AVATAR: &str = "SetAvatar";
    pub const CLEAR_AVATAR: &str = "ClearAvatar";
    pub const GET_SETTING: &str = "GetSetting";
    pub const LIST_SETTINGS: &str = "ListSettings";
    pub const LIST_USERS: &str = "ListUsers";
    pub const GET_USER: &str = "GetUser";
    pub const LIST_CANDIDATES: &str = "ListCandidates";
    pub const GET_DISCOVERY_STATUS: &str = "GetDiscoveryStatus";
    pub const FORGET_USER: &str = "ForgetUser";
    pub const SEND_MESSAGE: &str = "SendMessage";
    pub const LIST_MESSAGES: &str = "ListMessages";
    pub const LIST_CONVERSATION_SUMMARIES: &str = "ListConversationSummaries";
    pub const COUNT_MESSAGES: &str = "CountMessages";
    pub const MARK_READ: &str = "MarkRead";
    pub const MARK_CONVERSATION_READ: &str = "MarkConversationRead";
    pub const SET_TYPING: &str = "SetTyping";
    pub const GET_MESSAGE_STATE: &str = "GetMessageState";
    pub const SET_USER_POLICY: &str = "SetUserPolicy";
    pub const SET_DND: &str = "SetDnd";
    pub const SEND_ATTACHMENT: &str = "SendAttachment";
    pub const GET_TRANSFER_STATE: &str = "GetTransferState";
    pub const CANCEL_TRANSFER: &str = "CancelTransfer";
    pub const SAVE_ATTACHMENT: &str = "SaveAttachment";
    pub const GET_ATTACHMENT_PATH: &str = "GetAttachmentPath";
    pub const START_CALL: &str = "StartCall";
    pub const ACCEPT_CALL: &str = "AcceptCall";
    pub const DECLINE_CALL: &str = "DeclineCall";
    pub const HANGUP_CALL: &str = "HangupCall";
    pub const SET_CALL_MUTED: &str = "SetCallMuted";
    pub const GET_CALL_STATE: &str = "GetCallState";
    pub const LIST_CALL_LOGS: &str = "ListCallLogs";
    pub const LIST_AUDIO_DEVICES: &str = "ListAudioDevices";
    pub const START_WATCH_PARTY: &str = "StartWatchParty";
    pub const ACCEPT_WATCH_PARTY: &str = "AcceptWatchParty";
    pub const DECLINE_WATCH_PARTY: &str = "DeclineWatchParty";
    pub const INVITE_WATCH_PARTY: &str = "InviteWatchParty";
    pub const LEAVE_WATCH_PARTY: &str = "LeaveWatchParty";
    pub const PUBLISH_WATCH_PARTY: &str = "PublishWatchParty";
    pub const END_WATCH_PARTY: &str = "EndWatchParty";
    pub const GET_WATCH_PARTY: &str = "GetWatchParty";
    pub const GET_CALL_METRICS: &str = "GetCallMetrics";
    pub const REQUEST_REMOTE_SESSION: &str = "RequestRemoteSession";
    pub const ACCEPT_REMOTE_SESSION: &str = "AcceptRemoteSession";
    pub const DENY_REMOTE_SESSION: &str = "DenyRemoteSession";
    pub const END_REMOTE_SESSION: &str = "EndRemoteSession";
    pub const GET_REMOTE_SESSION: &str = "GetRemoteSession";
    pub const SHUTDOWN: &str = "Shutdown";
}

pub mod signals {
    pub const DAEMON_STARTED: &str = "DaemonStarted";
    pub const DAEMON_STOPPING: &str = "DaemonStopping";
    pub const SETTINGS_CHANGED: &str = "SettingsChanged";
    pub const CANDIDATE_FOUND: &str = "CandidateFound";
    pub const CANDIDATE_LOST: &str = "CandidateLost";
    pub const USER_CONNECTED: &str = "UserConnected";
    pub const USER_DISCONNECTED: &str = "UserDisconnected";
    pub const MESSAGE_RECEIVED: &str = "MessageReceived";
    pub const MESSAGE_STATE_CHANGED: &str = "MessageStateChanged";
    pub const CONVERSATION_CHANGED: &str = "ConversationChanged";
    pub const TYPING_CHANGED: &str = "TypingChanged";
    pub const USER_PROFILE_CHANGED: &str = "UserProfileChanged";
    pub const SELF_PROFILE_CHANGED: &str = "SelfProfileChanged";
    pub const SYNC_COMPLETED: &str = "SyncCompleted";
    pub const POLICY_CHANGED: &str = "PolicyChanged";
    pub const TRANSFER_PROGRESS: &str = "TransferProgress";
    pub const TRANSFER_DONE: &str = "TransferDone";
    pub const TRANSFER_FAILED: &str = "TransferFailed";
    pub const INCOMING_CALL: &str = "IncomingCall";
    pub const CALL_STATE_CHANGED: &str = "CallStateChanged";
    pub const CALL_MUTE_CHANGED: &str = "CallMuteChanged";
    pub const CALL_ENDED: &str = "CallEnded";
    pub const REMOTE_SESSION_REQUESTED: &str = "RemoteSessionRequested";
    pub const REMOTE_SESSION_STARTED: &str = "RemoteSessionStarted";
    pub const REMOTE_SESSION_ENDED: &str = "RemoteSessionEnded";
    pub const REMOTE_INDICATOR_CHANGED: &str = "RemoteIndicatorChanged";
    pub const WATCH_PARTY_REQUESTED: &str = "WatchPartyRequested";
    pub const WATCH_PARTY_CHANGED: &str = "WatchPartyChanged";
    pub const WATCH_PARTY_ENDED: &str = "WatchPartyEnded";
}

pub mod errors {
    pub const FAILED: &str = "org.ticonnect.Daemon.Error.Failed";
    pub const NOT_FOUND: &str = "org.ticonnect.Daemon.Error.NotFound";
}

pub mod client;
pub mod events;

#[cfg(test)]
mod bus_tests {
    use std::ffi::OsString;

    use super::ipc_bus_address_from;

    #[test]
    fn missing_address_uses_session_bus() {
        assert_eq!(ipc_bus_address_from(None), None);
    }

    #[test]
    fn empty_address_uses_session_bus() {
        assert_eq!(ipc_bus_address_from(Some(OsString::new())), None);
    }

    #[test]
    fn explicit_address_is_used() {
        assert_eq!(
            ipc_bus_address_from(Some(OsString::from("unix:path=/tmp/ticonnect-bus"))).as_deref(),
            Some("unix:path=/tmp/ticonnect-bus")
        );
    }
}
