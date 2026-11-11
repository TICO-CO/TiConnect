use std::sync::Arc;

use anyhow::{Context, Result};
use ticonnect_core::ipc;
use ticonnect_net::discovery::Discovery;
use tokio::signal::unix::{SignalKind, signal};
use tokio::sync::watch;
use zbus::object_server::SignalEmitter;

use crate::config_watch;
use crate::discovery_service;
use crate::pairing_service;
use crate::reconnect_service;
use crate::service::{DaemonService, DaemonState};

#[derive(Clone)]
pub struct Shutdown {
    requested: watch::Sender<bool>,
}

impl Shutdown {
    pub fn new() -> Self {
        let (requested, _receiver) = watch::channel(false);
        Self { requested }
    }

    pub fn request(&self) {
        self.requested.send_replace(true);
    }

    pub fn is_requested(&self) -> bool {
        *self.requested.borrow()
    }

    pub async fn wait(&self) {
        let mut receiver = self.requested.subscribe();
        if *receiver.borrow() {
            return;
        }
        while receiver.changed().await.is_ok() {
            if *receiver.borrow() {
                return;
            }
        }
    }
}

impl Default for Shutdown {
    fn default() -> Self {
        Self::new()
    }
}

pub async fn run(state: Arc<DaemonState>) -> Result<()> {
    let service = DaemonService::new(state.clone());

    let connection = zbus::connection::Builder::session()
        .name(ipc::DAEMON_BUS_NAME)
        .allow_name_replacements(false)
        .replace_existing_names(false)
        .serve_at(ipc::DAEMON_PATH, service)
        .build()
        .await
        .context("acquire daemon bus name")?;

    tracing::info!(bus = ipc::DAEMON_BUS_NAME, "daemon started");
    emit_started(&connection).await?;

    let watcher =
        config_watch::spawn(connection.clone(), state.clone()).context("start config watcher")?;
    if let Err(error) = state.discovery().start() {
        tracing::warn!(%error, "failed to start discovery");
    }
    let discovery_task = discovery_service::spawn(connection.clone(), state.clone());
    let pairing_task = pairing_service::spawn(connection.clone(), state.pairing().clone());
    let reconnect_task = reconnect_service::spawn(
        state.transport().clone(),
        state.discovery().clone(),
        state.trust().clone(),
    );

    wait_for_shutdown(&connection, state.shutdown()).await?;
    watcher.abort();
    state.discovery().stop_and_wait().await;
    discovery_task.abort();
    pairing_task.abort();
    reconnect_task.abort();
    state.transport().shutdown().await;

    if let Err(error) = emit_stopping(&connection).await {
        tracing::warn!(%error, "failed to emit DaemonStopping");
    }
    if let Err(error) = connection.release_name(ipc::DAEMON_BUS_NAME).await {
        tracing::warn!(%error, "failed to release daemon bus name");
    }
    tracing::info!("daemon stopped");
    Ok(())
}

async fn emit_started(connection: &zbus::Connection) -> Result<()> {
    let emitter =
        SignalEmitter::new(connection, ipc::DAEMON_PATH).context("create daemon signal emitter")?;
    DaemonService::daemon_started(emitter)
        .await
        .context("emit DaemonStarted")
}

async fn emit_stopping(connection: &zbus::Connection) -> Result<()> {
    let emitter =
        SignalEmitter::new(connection, ipc::DAEMON_PATH).context("create daemon signal emitter")?;
    DaemonService::daemon_stopping(emitter)
        .await
        .context("emit DaemonStopping")
}

async fn wait_for_shutdown(connection: &zbus::Connection, shutdown: &Shutdown) -> Result<()> {
    let mut terminate = signal(SignalKind::terminate()).context("install SIGTERM handler")?;
    let mut interrupt = signal(SignalKind::interrupt()).context("install SIGINT handler")?;

    tokio::select! {
        _ = shutdown.wait() => tracing::info!("shutdown requested"),
        _ = terminate.recv() => tracing::info!("received SIGTERM"),
        _ = interrupt.recv() => tracing::info!("received SIGINT"),
        _ = connection.closed() => tracing::warn!("daemon bus connection closed"),
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn shutdown_wait_wakes_after_request() {
        let shutdown = Shutdown::new();
        let waiter = tokio::spawn({
            let shutdown = shutdown.clone();
            async move { shutdown.wait().await }
        });

        shutdown.request();
        waiter.await.expect("shutdown wait");
        assert!(shutdown.is_requested());
    }

    #[tokio::test]
    async fn shutdown_wait_returns_when_already_requested() {
        let shutdown = Shutdown::new();
        shutdown.request();
        shutdown.wait().await;
    }
}
