use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::Instant;

use ticonnect_core::ipc;
use ticonnect_settings::{SettingValue, Settings};
use zbus::interface;
use zbus::object_server::SignalEmitter;
use zbus::{DBusError, OwnedVariant, Str};

use crate::identity::PublicIdentity;
use crate::runtime::Shutdown;
use crate::storage::{DatabaseHandle, Peer, StorageError};

#[derive(Debug, DBusError)]
#[zbus(prefix = "org.ticonnect.Daemon.Error")]
pub enum ServiceError {
    Failed(String),
    NotFound(String),
    #[zbus(error)]
    Zbus(zbus::Error),
}

pub struct DaemonState {
    started_at: Instant,
    shutdown: Arc<Shutdown>,
    identity: PublicIdentity,
    settings: RwLock<Settings>,
    database: DatabaseHandle,
}

impl DaemonState {
    pub fn new(
        shutdown: Arc<Shutdown>,
        identity: PublicIdentity,
        settings: Settings,
        database: DatabaseHandle,
    ) -> Self {
        Self {
            started_at: Instant::now(),
            shutdown,
            identity,
            settings: RwLock::new(settings),
            database,
        }
    }

    pub fn shutdown(&self) -> &Arc<Shutdown> {
        &self.shutdown
    }

    pub fn identity(&self) -> &PublicIdentity {
        &self.identity
    }

    pub fn database(&self) -> &DatabaseHandle {
        &self.database
    }

    pub fn settings(&self) -> Settings {
        self.settings
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    pub fn replace_settings(&self, settings: Settings) {
        *self
            .settings
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = settings;
    }
}

pub struct DaemonService {
    state: Arc<DaemonState>,
}

impl DaemonService {
    pub fn new(state: Arc<DaemonState>) -> Self {
        Self { state }
    }

    async fn status(&self) -> Result<HashMap<String, OwnedVariant>, ServiceError> {
        let peer_count = self
            .state
            .database()
            .count_peers()
            .await
            .map_err(database_error)?;
        let outbox_depth = self
            .state
            .database()
            .outbox_depth()
            .await
            .map_err(database_error)?;

        let mut status = HashMap::new();
        status.insert(
            ipc::STATUS_UPTIME_SECONDS.to_owned(),
            OwnedVariant::from(self.state.started_at.elapsed().as_secs()),
        );
        status.insert(
            ipc::STATUS_PEER_COUNT.to_owned(),
            OwnedVariant::from(peer_count),
        );
        status.insert(
            ipc::STATUS_OUTBOX_DEPTH.to_owned(),
            OwnedVariant::from(outbox_depth),
        );
        status.insert(
            ipc::STATUS_TRANSPORT_STATE.to_owned(),
            OwnedVariant::from(Str::from(ipc::TRANSPORT_OFFLINE)),
        );
        Ok(status)
    }

    fn identity(&self) -> HashMap<String, OwnedVariant> {
        let settings = self.state.settings();
        let identity = self.state.identity();
        HashMap::from([
            (
                ipc::IDENTITY_NODE_ID.to_owned(),
                string_variant(identity.node_id().to_string()),
            ),
            (
                ipc::IDENTITY_FINGERPRINT.to_owned(),
                string_variant(identity.fingerprint().to_hex()),
            ),
            (
                ipc::IDENTITY_DISPLAY_NAME.to_owned(),
                string_variant(settings.identity_display_name().to_owned()),
            ),
        ])
    }
}

#[interface(name = "org.ticonnect.Daemon")]
impl DaemonService {
    fn ping(&self) -> &'static str {
        "pong"
    }

    fn get_version(&self) -> &'static str {
        ipc::VERSION
    }

    async fn get_status(&self) -> Result<HashMap<String, OwnedVariant>, ServiceError> {
        self.status().await
    }

    async fn get_identity(&self) -> HashMap<String, OwnedVariant> {
        self.identity()
    }

    async fn get_setting(&self, key: &str) -> Result<OwnedVariant, ServiceError> {
        let settings = self.state.settings();
        match settings.get_setting(key) {
            Ok(value) => Ok(setting_variant(value)),
            Err(ticonnect_settings::Error::UnknownSetting(_)) => {
                Err(ServiceError::NotFound(format!("unknown setting: {key}")))
            }
            Err(error) => Err(ServiceError::Failed(error.to_string())),
        }
    }

    async fn list_settings(&self) -> Result<HashMap<String, OwnedVariant>, ServiceError> {
        let settings = self.state.settings();
        settings
            .list_settings()
            .map(|values| {
                values
                    .into_iter()
                    .map(|(key, value)| (key, setting_variant(value)))
                    .collect()
            })
            .map_err(|error| ServiceError::Failed(error.to_string()))
    }

    async fn list_peers(&self) -> Result<Vec<HashMap<String, OwnedVariant>>, ServiceError> {
        let peers = self
            .state
            .database()
            .list_peers()
            .await
            .map_err(database_error)?;
        Ok(peers.into_iter().map(peer_variant).collect())
    }

    fn shutdown(&self) -> Result<(), ServiceError> {
        if self.state.shutdown().is_requested() {
            return Err(ServiceError::Failed(
                "daemon is already shutting down".to_owned(),
            ));
        }
        tracing::info!("daemon shutdown requested over D-Bus");
        self.state.shutdown().request();
        Ok(())
    }

    #[zbus(property)]
    fn version(&self) -> &'static str {
        ipc::VERSION
    }

    #[zbus(signal)]
    pub async fn daemon_started(emitter: SignalEmitter<'_>) -> zbus::Result<()>;

    #[zbus(signal)]
    pub async fn daemon_stopping(emitter: SignalEmitter<'_>) -> zbus::Result<()>;

    #[zbus(signal)]
    pub async fn settings_changed(emitter: SignalEmitter<'_>, key: &str) -> zbus::Result<()>;
}

fn setting_variant(value: SettingValue) -> OwnedVariant {
    match value {
        SettingValue::String(value) => string_variant(value),
        SettingValue::Bool(value) => OwnedVariant::from(value),
        SettingValue::Integer(value) => OwnedVariant::from(value),
        SettingValue::Path(Some(path)) => string_variant(path.display().to_string()),
        SettingValue::Path(None) => string_variant(String::new()),
    }
}

fn string_variant(value: String) -> OwnedVariant {
    OwnedVariant::from(Str::from(value))
}

fn peer_variant(peer: Peer) -> HashMap<String, OwnedVariant> {
    HashMap::from([
        (ipc::PEER_ID.to_owned(), string_variant(peer.id)),
        (
            ipc::PEER_FINGERPRINT.to_owned(),
            string_variant(peer.fingerprint),
        ),
        (
            ipc::PEER_DISPLAY_NAME.to_owned(),
            string_variant(peer.display_name.unwrap_or_default()),
        ),
        (
            ipc::PEER_POLICY.to_owned(),
            string_variant(peer.policy.as_str().to_owned()),
        ),
        (
            ipc::PEER_TRUSTED_AT.to_owned(),
            OwnedVariant::from(u64::try_from(peer.trusted_at).unwrap_or_default()),
        ),
        (
            ipc::PEER_LAST_SEEN.to_owned(),
            OwnedVariant::from(
                u64::try_from(peer.last_seen.unwrap_or_default()).unwrap_or_default(),
            ),
        ),
    ])
}

fn database_error(error: StorageError) -> ServiceError {
    tracing::error!(%error, "database request failed");
    ServiceError::Failed("database request failed".to_owned())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::identity::Identity;
    use crate::storage::DatabaseHandle;

    #[tokio::test]
    async fn ping_and_version_are_stable() {
        let (service, _dir) = test_service().await;
        assert_eq!(service.ping(), "pong");
        assert_eq!(service.get_version(), ipc::VERSION);
        assert_eq!(service.version(), ipc::VERSION);
    }

    #[tokio::test]
    async fn status_has_stable_keys_and_values() {
        let (service, _dir) = test_service().await;
        let status = service.status().await.expect("status");

        assert!(status.contains_key(ipc::STATUS_UPTIME_SECONDS));
        assert_eq!(
            u32::try_from(&status[ipc::STATUS_PEER_COUNT]).expect("peer count"),
            0
        );
        assert_eq!(
            u32::try_from(&status[ipc::STATUS_OUTBOX_DEPTH]).expect("outbox depth"),
            0
        );
        assert_eq!(
            <&str>::try_from(&status[ipc::STATUS_TRANSPORT_STATE]).expect("transport state"),
            ipc::TRANSPORT_OFFLINE
        );
    }

    #[tokio::test]
    async fn identity_and_settings_are_exposed() {
        let (service, _dir) = test_service().await;
        let identity = service.get_identity().await;
        assert!(identity.contains_key(ipc::IDENTITY_NODE_ID));
        assert!(identity.contains_key(ipc::IDENTITY_FINGERPRINT));

        let settings = service.list_settings().await.expect("settings");
        assert!(settings.contains_key("dnd.enabled"));
        assert!(
            !bool::try_from(service.get_setting("dnd.enabled").await.expect("dnd")).expect("bool")
        );
    }

    #[tokio::test]
    async fn shutdown_requests_runtime_stop() {
        let (service, _dir) = test_service().await;
        let shorthand = service.state.shutdown().clone();
        let waiter = tokio::spawn({
            let shutdown = shorthand.clone();
            async move { shutdown.wait().await }
        });

        service.shutdown().expect("shutdown request");
        waiter.await.expect("shutdown wait");
        assert!(shorthand.is_requested());
        assert!(service.shutdown().is_err());
    }

    async fn test_service() -> (DaemonService, PathBuf) {
        let dir = std::env::temp_dir().join(format!(
            "ticonnect-service-{}-{}",
            std::process::id(),
            unique_id()
        ));
        std::fs::create_dir_all(&dir).expect("create test dir");
        let settings = Settings::load_from(dir.join("config.toml")).expect("settings");
        let identity = Identity::load_or_create(&dir).expect("identity").public();
        let database = DatabaseHandle::spawn(dir.join("ticonnect.db"))
            .await
            .expect("database");
        let state = Arc::new(DaemonState::new(
            Arc::new(Shutdown::new()),
            identity,
            settings,
            database,
        ));
        (DaemonService::new(state), dir)
    }

    fn unique_id() -> u64 {
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT: AtomicU64 = AtomicU64::new(0);
        NEXT.fetch_add(1, Ordering::Relaxed)
    }
}
