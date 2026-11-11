use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use notify::{RecursiveMode, Watcher};
use thiserror::Error;
use ticonnect_core::ipc;
use ticonnect_settings::Settings;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use zbus::object_server::SignalEmitter;

use crate::service::{DaemonService, DaemonState};

#[derive(Debug, Error)]
pub enum WatchError {
    #[error(transparent)]
    Notify(#[from] notify::Error),

    #[error(transparent)]
    Io(#[from] std::io::Error),
}

pub fn spawn(
    connection: zbus::Connection,
    state: Arc<DaemonState>,
) -> Result<JoinHandle<()>, WatchError> {
    let config_file = state.settings().config_file().to_path_buf();
    let config_dir = config_file
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .map_or_else(|| PathBuf::from("."), Path::to_path_buf);
    let config_name = config_file.file_name().map(|name| name.to_os_string());
    let (sender, mut receiver) = mpsc::unbounded_channel();

    let mut watcher = notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
        let Ok(event) = event else {
            return;
        };
        let matches = event
            .paths
            .iter()
            .any(|candidate| candidate.file_name() == config_name.as_deref());
        if matches {
            let _ = sender.send(());
        }
    })?;
    watcher.watch(&config_dir, RecursiveMode::NonRecursive)?;

    Ok(tokio::spawn(async move {
        let _watcher = watcher;
        let mut last = read_bytes(&config_file);
        while receiver.recv().await.is_some() {
            tokio::time::sleep(Duration::from_millis(100)).await;
            while receiver.try_recv().is_ok() {}

            let next = read_bytes(&config_file);
            if next == last {
                continue;
            }
            last = next;

            let path = config_file.clone();
            let loaded = tokio::task::spawn_blocking(move || Settings::load_from(path)).await;
            match loaded {
                Ok(Ok(new_settings)) => {
                    reloaded(&connection, &state, new_settings).await;
                }
                Ok(Err(error)) => {
                    tracing::warn!(%error, "failed to reload settings; keeping previous values");
                }
                Err(error) => {
                    tracing::warn!(%error, "settings reload task failed");
                }
            }
        }
    }))
}

async fn reloaded(connection: &zbus::Connection, state: &DaemonState, new_settings: Settings) {
    let old_values = match state.settings().list_settings() {
        Ok(values) => values,
        Err(error) => {
            tracing::warn!(%error, "failed to snapshot previous settings");
            return;
        }
    };
    let new_values = match new_settings.list_settings() {
        Ok(values) => values,
        Err(error) => {
            tracing::warn!(%error, "failed to read reloaded settings");
            return;
        }
    };

    let discovery_changed = new_values
        .iter()
        .any(|(key, value)| key.starts_with("discovery.") && old_values.get(key) != Some(value));

    state.replace_settings(new_settings);
    if discovery_changed {
        state.reconfigure_discovery().await;
    }

    for (key, value) in new_values {
        if old_values.get(&key) == Some(&value) {
            continue;
        }

        tracing::info!(setting = %key, "settings changed");
        let emitter = match SignalEmitter::new(connection, ipc::DAEMON_PATH) {
            Ok(emitter) => emitter,
            Err(error) => {
                tracing::warn!(%error, "failed to create settings signal emitter");
                continue;
            }
        };
        if let Err(error) = DaemonService::settings_changed(emitter, &key).await {
            tracing::warn!(%error, setting = %key, "failed to emit SettingsChanged");
        }
    }
}

fn read_bytes(path: &Path) -> Option<Vec<u8>> {
    std::fs::read(path).ok()
}
