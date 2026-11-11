use ticonnect_logging::{LogSettings, Logging};
use ticonnect_settings::Settings;

use crate::{Error, Result};

pub fn init() -> Result<Settings> {
    let settings = Settings::load()?;
    let log_settings = LogSettings::new(
        settings.log_level(),
        settings.log_file(),
        settings.log_format(),
    )
    .map_err(|error| Error::Runtime(format!("invalid logging settings: {error}")))?;
    Logging::init(&log_settings)?;
    Ok(settings)
}
