pub const DAEMON_BUS_NAME: &str = "org.ticonnect.Daemon";
pub const DAEMON_PATH: &str = "/org/ticonnect/Daemon";
pub const DAEMON_INTERFACE: &str = "org.ticonnect.Daemon";
pub const UI_BUS_NAME: &str = "org.ticonnect.UI";
pub const UI_PATH: &str = "/org/ticonnect/UI";
pub const UI_INTERFACE: &str = "org.ticonnect.UI";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub const STATUS_UPTIME_SECONDS: &str = "uptime_seconds";
pub const STATUS_PEER_COUNT: &str = "peer_count";
pub const STATUS_OUTBOX_DEPTH: &str = "outbox_depth";
pub const STATUS_TRANSPORT_STATE: &str = "transport_state";
pub const STATUS_ACTIVE_SESSIONS: &str = "active_sessions";
pub const TRANSPORT_OFFLINE: &str = "offline";
pub const TRANSPORT_IDLE: &str = "idle";
pub const TRANSPORT_ONLINE: &str = "online";

pub const IDENTITY_NODE_ID: &str = "node_id";
pub const IDENTITY_FINGERPRINT: &str = "fingerprint";
pub const IDENTITY_DISPLAY_NAME: &str = "display_name";

pub const PEER_ID: &str = "id";
pub const PEER_FINGERPRINT: &str = "fingerprint";
pub const PEER_DISPLAY_NAME: &str = "display_name";
pub const PEER_POLICY: &str = "policy";
pub const PEER_TRUSTED_AT: &str = "trusted_at";
pub const PEER_LAST_SEEN: &str = "last_seen";
pub const PEER_ADDRESS: &str = "address";
pub const PEER_CONNECTED: &str = "connected";

pub const PAIRING_PIN: &str = "pin";
pub const PAIRING_QR: &str = "qr";
pub const PAIRING_EXPIRES_AT: &str = "expires_at";
pub const PAIRING_NODE_ID: &str = "node_id";
pub const PAIRING_PORT: &str = "port";

pub const CANDIDATE_NODE_ID: &str = "node_id";
pub const CANDIDATE_FINGERPRINT: &str = "fingerprint";
pub const CANDIDATE_DISPLAY_NAME: &str = "display_name";
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
pub const MESSAGE_PEER_ID: &str = "peer_id";
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
pub const REMOTE_PEER_ID: &str = "peer_id";
pub const REMOTE_FINGERPRINT: &str = "fingerprint";
pub const REMOTE_ROLE: &str = "role";
pub const REMOTE_STATE: &str = "state";
pub const REMOTE_PIN: &str = "pin";
pub const REMOTE_INDICATOR: &str = "indicator";
pub const REMOTE_EXPIRES_AT: &str = "expires_at";
pub const CALL_ID: &str = "call_id";
pub const CALL_PEER_ID: &str = "peer_id";
pub const CALL_DIRECTION: &str = "direction";
pub const CALL_STATE: &str = "state";
pub const CALL_STARTED_AT: &str = "started_at";
pub const CALL_CONNECTED_AT: &str = "connected_at";
pub const CALL_REASON: &str = "reason";
pub const TRANSFER_HASH: &str = "hash";
pub const TRANSFER_PEER_ID: &str = "peer_id";
pub const TRANSFER_NAME: &str = "name";
pub const TRANSFER_SIZE: &str = "size";
pub const TRANSFER_BYTES: &str = "bytes";
pub const TRANSFER_PHASE: &str = "phase";
pub const TRANSFER_REASON: &str = "reason";
pub const MESSAGE_STATE: &str = "state";
pub const MESSAGE_NOTIFY: &str = "notify";

pub mod methods {
    pub const PING: &str = "Ping";
    pub const GET_VERSION: &str = "GetVersion";
    pub const GET_STATUS: &str = "GetStatus";
    pub const GET_IDENTITY: &str = "GetIdentity";
    pub const GET_SETTING: &str = "GetSetting";
    pub const LIST_SETTINGS: &str = "ListSettings";
    pub const LIST_PEERS: &str = "ListPeers";
    pub const LIST_CANDIDATES: &str = "ListCandidates";
    pub const GET_DISCOVERY_STATUS: &str = "GetDiscoveryStatus";
    pub const START_PAIRING: &str = "StartPairing";
    pub const PAIR_WITH_PIN: &str = "PairWithPin";
    pub const PAIR_WITH_QR: &str = "PairWithQr";
    pub const CANCEL_PAIRING: &str = "CancelPairing";
    pub const FORGET_PEER: &str = "ForgetPeer";
    pub const LIST_TRUSTED_PEERS: &str = "ListTrustedPeers";
    pub const SEND_MESSAGE: &str = "SendMessage";
    pub const LIST_MESSAGES: &str = "ListMessages";
    pub const COUNT_MESSAGES: &str = "CountMessages";
    pub const MARK_READ: &str = "MarkRead";
    pub const GET_MESSAGE_STATE: &str = "GetMessageState";
    pub const SET_PEER_POLICY: &str = "SetPeerPolicy";
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
    pub const GET_CALL_STATE: &str = "GetCallState";
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
    pub const PAIRING_STARTED: &str = "PairingStarted";
    pub const PAIRING_COMPLETED: &str = "PairingCompleted";
    pub const PAIRING_FAILED: &str = "PairingFailed";
    pub const PEER_CONNECTED: &str = "PeerConnected";
    pub const PEER_DISCONNECTED: &str = "PeerDisconnected";
    pub const MESSAGE_RECEIVED: &str = "MessageReceived";
    pub const MESSAGE_STATE_CHANGED: &str = "MessageStateChanged";
    pub const SYNC_COMPLETED: &str = "SyncCompleted";
    pub const POLICY_CHANGED: &str = "PolicyChanged";
    pub const TRANSFER_PROGRESS: &str = "TransferProgress";
    pub const TRANSFER_DONE: &str = "TransferDone";
    pub const TRANSFER_FAILED: &str = "TransferFailed";
    pub const INCOMING_CALL: &str = "IncomingCall";
    pub const CALL_STATE_CHANGED: &str = "CallStateChanged";
    pub const CALL_ENDED: &str = "CallEnded";
    pub const REMOTE_SESSION_REQUESTED: &str = "RemoteSessionRequested";
    pub const REMOTE_SESSION_STARTED: &str = "RemoteSessionStarted";
    pub const REMOTE_SESSION_ENDED: &str = "RemoteSessionEnded";
    pub const REMOTE_INDICATOR_CHANGED: &str = "RemoteIndicatorChanged";
}

pub mod errors {
    pub const FAILED: &str = "org.ticonnect.Daemon.Error.Failed";
    pub const NOT_FOUND: &str = "org.ticonnect.Daemon.Error.NotFound";
}

pub mod client;
pub mod events;
