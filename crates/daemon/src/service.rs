use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use ticonnect_core::ipc;
use zbus::interface;
use zbus::object_server::SignalEmitter;
use zbus::{DBusError, OwnedVariant, Str};

#[derive(Debug, DBusError)]
#[zbus(prefix = "org.ticonnect.Daemon.Error")]
pub enum ServiceError {
    Failed(String),
    #[zbus(error)]
    Zbus(zbus::Error),
}

use crate::runtime::Shutdown;

pub struct DaemonService {
    started_at: Instant,
    shutdown: Arc<Shutdown>,
}

impl DaemonService {
    pub fn new(shutdown: Arc<Shutdown>) -> Self {
        Self {
            started_at: Instant::now(),
            shutdown,
        }
    }

    fn status(&self) -> HashMap<String, OwnedVariant> {
        let mut status = HashMap::new();
        status.insert(
            ipc::STATUS_UPTIME_SECONDS.to_owned(),
            OwnedVariant::from(self.started_at.elapsed().as_secs()),
        );
        status.insert(ipc::STATUS_PEER_COUNT.to_owned(), OwnedVariant::from(0_u32));
        status.insert(
            ipc::STATUS_OUTBOX_DEPTH.to_owned(),
            OwnedVariant::from(0_u32),
        );
        status.insert(
            ipc::STATUS_TRANSPORT_STATE.to_owned(),
            OwnedVariant::from(Str::from(ipc::TRANSPORT_OFFLINE)),
        );
        status
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

    fn get_status(&self) -> HashMap<String, OwnedVariant> {
        self.status()
    }

    fn shutdown(&self) -> Result<(), ServiceError> {
        if self.shutdown.is_requested() {
            return Err(ServiceError::Failed(
                "daemon is already shutting down".to_owned(),
            ));
        }
        tracing::info!("daemon shutdown requested over D-Bus");
        self.shutdown.request();
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ping_and_version_are_stable() {
        let service = DaemonService::new(Arc::new(Shutdown::new()));
        assert_eq!(service.ping(), "pong");
        assert_eq!(service.get_version(), ipc::VERSION);
        assert_eq!(service.version(), ipc::VERSION);
    }

    #[test]
    fn status_has_stable_keys_and_values() {
        let service = DaemonService::new(Arc::new(Shutdown::new()));
        let status = service.status();

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
    async fn shutdown_requests_runtime_stop() {
        let shutdown = Arc::new(Shutdown::new());
        let service = DaemonService::new(shutdown.clone());
        let waiter = tokio::spawn({
            let shutdown = shutdown.clone();
            async move { shutdown.wait().await }
        });

        service.shutdown().expect("shutdown request");
        waiter.await.expect("shutdown wait");
        assert!(shutdown.is_requested());
        assert!(service.shutdown().is_err());
    }
}
