#![forbid(unsafe_code)]

use anyhow::Context;

fn main() -> anyhow::Result<()> {
    run()
}

fn run() -> anyhow::Result<()> {
    let settings = ticonnect_settings::Settings::load().context("failed to load settings")?;
    let log_settings = ticonnect_logging::LogSettings::new(
        settings.log_level(),
        settings.log_file(),
        settings.log_format(),
    )
    .context("invalid logging settings")?;
    ticonnect_logging::Logging::init(&log_settings).context("failed to initialize logging")?;

    Ok(())
}
