use std::sync::Arc;

use ticonnect_core::ipc;
use ticonnect_net::discovery::{Discovery, DiscoveryEvent};
use tokio::task::JoinHandle;
use zbus::object_server::SignalEmitter;

use crate::service::{DaemonService, DaemonState, candidate_variant};

pub fn spawn(connection: zbus::Connection, state: Arc<DaemonState>) -> JoinHandle<()> {
    let mut events = state.discovery().subscribe();

    tokio::spawn(async move {
        loop {
            match events.recv().await {
                Ok(DiscoveryEvent::Found(candidate)) => {
                    let emitter = match SignalEmitter::new(&connection, ipc::DAEMON_PATH) {
                        Ok(emitter) => emitter,
                        Err(error) => {
                            tracing::warn!(%error, "failed to create candidate signal emitter");
                            continue;
                        }
                    };
                    if let Err(error) =
                        DaemonService::candidate_found(emitter, candidate_variant(candidate)).await
                    {
                        tracing::warn!(%error, "failed to emit CandidateFound");
                    }
                }
                Ok(DiscoveryEvent::Lost(fingerprint)) => {
                    let emitter = match SignalEmitter::new(&connection, ipc::DAEMON_PATH) {
                        Ok(emitter) => emitter,
                        Err(error) => {
                            tracing::warn!(%error, "failed to create candidate signal emitter");
                            continue;
                        }
                    };
                    if let Err(error) = DaemonService::candidate_lost(emitter, &fingerprint).await {
                        tracing::warn!(%error, "failed to emit CandidateLost");
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(skipped)) => {
                    tracing::warn!(skipped, "discovery event stream lagged");
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            }
        }
    })
}
