pub const DAEMON_BUS_NAME: &str = "org.ticonnect.Daemon";
pub const DAEMON_PATH: &str = "/org/ticonnect/Daemon";
pub const DAEMON_INTERFACE: &str = "org.ticonnect.Daemon";
pub const UI_INTERFACE: &str = "org.ticonnect.UI";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub const STATUS_UPTIME_SECONDS: &str = "uptime_seconds";
pub const STATUS_PEER_COUNT: &str = "peer_count";
pub const STATUS_OUTBOX_DEPTH: &str = "outbox_depth";
pub const STATUS_TRANSPORT_STATE: &str = "transport_state";
pub const TRANSPORT_OFFLINE: &str = "offline";

pub const IDENTITY_NODE_ID: &str = "node_id";
pub const IDENTITY_FINGERPRINT: &str = "fingerprint";
pub const IDENTITY_DISPLAY_NAME: &str = "display_name";

pub const PEER_ID: &str = "id";
pub const PEER_FINGERPRINT: &str = "fingerprint";
pub const PEER_DISPLAY_NAME: &str = "display_name";
pub const PEER_POLICY: &str = "policy";
pub const PEER_TRUSTED_AT: &str = "trusted_at";
pub const PEER_LAST_SEEN: &str = "last_seen";

pub mod methods {
    pub const PING: &str = "Ping";
    pub const GET_VERSION: &str = "GetVersion";
    pub const GET_STATUS: &str = "GetStatus";
    pub const GET_IDENTITY: &str = "GetIdentity";
    pub const GET_SETTING: &str = "GetSetting";
    pub const LIST_SETTINGS: &str = "ListSettings";
    pub const LIST_PEERS: &str = "ListPeers";
    pub const SHUTDOWN: &str = "Shutdown";
}

pub mod signals {
    pub const DAEMON_STARTED: &str = "DaemonStarted";
    pub const DAEMON_STOPPING: &str = "DaemonStopping";
    pub const SETTINGS_CHANGED: &str = "SettingsChanged";
}

pub mod errors {
    pub const FAILED: &str = "org.ticonnect.Daemon.Error.Failed";
    pub const NOT_FOUND: &str = "org.ticonnect.Daemon.Error.NotFound";
}
