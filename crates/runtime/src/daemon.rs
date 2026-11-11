use std::sync::Arc;

use ticonnect_daemon::identity::Identity;
use ticonnect_daemon::runtime::Shutdown;
use ticonnect_daemon::service::DaemonState;
use ticonnect_daemon::storage::DatabaseHandle;
use ticonnect_settings::Settings;

use crate::{Error, Result};

pub async fn run(settings: Settings) -> Result<()> {
    settings.ensure_config_dir()?;
    let data_dir = settings.data_dir()?;
    let identity = Identity::load_or_create(settings.config_dir())
        .map_err(Error::Identity)?
        .public();
    let database = DatabaseHandle::spawn(data_dir.join("ticonnect.db"))
        .await
        .map_err(Error::Storage)?;
    let state = Arc::new(DaemonState::new(
        Arc::new(Shutdown::new()),
        identity,
        settings,
        database,
    ));

    ticonnect_daemon::runtime::run(state).await?;
    Ok(())
}

pub async fn run_daemon() -> Result<()> {
    let mut supervisor = crate::supervisor::Supervisor::new();
    let settings = crate::bootstrap::init()?;

    supervisor.spawn("daemon", async move { run(settings).await });

    while let Some(result) = supervisor.join_next().await {
        result?;
    }

    Ok(())
}
