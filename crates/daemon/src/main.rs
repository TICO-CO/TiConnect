#![forbid(unsafe_code)]

use anyhow::Context;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let settings = ticonnect_settings::Settings::load().context("failed to load settings")?;
    let log_settings = ticonnect_logging::LogSettings::new(
        settings.log_level(),
        settings.log_file(),
        settings.log_format(),
    )
    .context("invalid logging settings")?;
    ticonnect_logging::Logging::init(&log_settings).context("failed to initialize logging")?;

    if let Err(error) = ticonnect_daemon::runtime::run().await {
        tracing::error!(%error, "daemon failed");
        return Err(error);
    }
    Ok(())
}
