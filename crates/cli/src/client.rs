use std::collections::HashMap;

use zbus::OwnedVariant;
use zbus::proxy;

#[proxy(
    default_service = "org.ticonnect.Daemon",
    default_path = "/org/ticonnect/Daemon",
    interface = "org.ticonnect.Daemon"
)]
pub trait Daemon {
    fn ping(&self) -> zbus::Result<String>;
    fn get_version(&self) -> zbus::Result<String>;
    fn get_status(&self) -> zbus::Result<HashMap<String, OwnedVariant>>;
    fn get_identity(&self) -> zbus::Result<HashMap<String, OwnedVariant>>;
    fn get_setting(&self, key: &str) -> zbus::Result<OwnedVariant>;
    fn list_settings(&self) -> zbus::Result<HashMap<String, OwnedVariant>>;
    fn list_peers(&self) -> zbus::Result<Vec<HashMap<String, OwnedVariant>>>;
    fn list_candidates(&self) -> zbus::Result<Vec<HashMap<String, OwnedVariant>>>;
    fn get_discovery_status(&self) -> zbus::Result<HashMap<String, OwnedVariant>>;
    fn start_pairing(&self) -> zbus::Result<HashMap<String, OwnedVariant>>;
    fn pair_with_pin(&self, node_id: &str, pin: &str) -> zbus::Result<bool>;
    fn pair_with_qr(&self, payload: &str) -> zbus::Result<bool>;
    fn cancel_pairing(&self) -> zbus::Result<()>;
    fn forget_peer(&self, peer_id: &str) -> zbus::Result<bool>;
    fn list_trusted_peers(&self) -> zbus::Result<Vec<HashMap<String, OwnedVariant>>>;
    fn shutdown(&self) -> zbus::Result<()>;

    #[zbus(signal)]
    fn candidate_found(&self, candidate: HashMap<String, OwnedVariant>) -> zbus::Result<()>;

    #[zbus(signal)]
    fn candidate_lost(&self, fingerprint: &str) -> zbus::Result<()>;

    #[zbus(signal)]
    fn pairing_started(&self, pairing: HashMap<String, OwnedVariant>) -> zbus::Result<()>;

    #[zbus(signal)]
    fn pairing_completed(&self, peer_id: &str, fingerprint: &str) -> zbus::Result<()>;

    #[zbus(signal)]
    fn pairing_failed(&self, reason: &str) -> zbus::Result<()>;

    #[zbus(signal)]
    fn peer_connected(&self, peer: HashMap<String, OwnedVariant>) -> zbus::Result<()>;

    #[zbus(signal)]
    fn peer_disconnected(&self, peer: HashMap<String, OwnedVariant>) -> zbus::Result<()>;
}
