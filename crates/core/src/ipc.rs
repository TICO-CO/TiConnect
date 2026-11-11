pub const DAEMON_BUS_NAME: &str = "org.ticonnect.Daemon";
pub const DAEMON_PATH: &str = "/org/ticonnect/Daemon";
pub const DAEMON_INTERFACE: &str = "org.ticonnect.Daemon";
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
}

pub mod errors {
    pub const FAILED: &str = "org.ticonnect.Daemon.Error.Failed";
    pub const NOT_FOUND: &str = "org.ticonnect.Daemon.Error.NotFound";
}

pub mod client;
