use std::sync::Mutex;

use tracing_subscriber::{layer::SubscriberExt, reload, util::SubscriberInitExt};

use crate::error::{Error, Result};
use crate::output::make_output;
use crate::settings::{self, LogSettings};
use crate::types::{FilterHandle, OutputHandle};

#[derive(Clone)]
pub struct Logging {
    filter: FilterHandle,
    output: OutputHandle,
}

static LOGGING: Mutex<Option<Logging>> = Mutex::new(None);

impl Logging {
    pub fn init(settings: &LogSettings) -> Result<Self> {
        let mut guard = LOGGING
            .lock()
            .map_err(|_| Error::Logging("logging initialization lock poisoned".to_owned()))?;

        if let Some(logging) = guard.as_ref() {
            return Ok(logging.clone());
        }

        let filter = settings::env_filter(settings);
        let (filter_layer, filter_handle) = reload::Layer::new(filter);
        let (output_layer, output_handle) = reload::Layer::new(make_output(settings)?);

        tracing_subscriber::registry()
            .with(filter_layer)
            .with(output_layer)
            .try_init()
            .map_err(|error| Error::Logging(error.to_string()))?;

        let logging = Logging {
            filter: filter_handle,
            output: output_handle,
        };
        *guard = Some(logging.clone());

        Ok(logging)
    }

    pub fn get() -> Option<Self> {
        match LOGGING.lock() {
            Ok(guard) => guard.clone(),
            Err(poisoned) => poisoned.into_inner().clone(),
        }
    }

    pub fn reload(&self, settings: &LogSettings) -> Result<()> {
        let filter = settings::env_filter(settings);
        let output = make_output(settings)?;

        self.output
            .reload(output)
            .map_err(|error| Error::Logging(error.to_string()))?;
        self.filter
            .reload(filter)
            .map_err(|error| Error::Logging(error.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use crate::{LogSettings, Logging};

    #[test]
    fn init_is_idempotent() {
        let settings = LogSettings::default();

        let first = Logging::init(&settings).expect("first init");
        let second = Logging::init(&settings).expect("second init");

        assert!(Logging::get().is_some());
        first.reload(&settings).expect("reload after second init");
        second.reload(&settings).expect("reload after second init");
    }

    #[test]
    fn reload_accepts_valid_config() {
        let logging = Logging::init(&LogSettings::default()).expect("init");
        let settings = LogSettings::new("warn", None, "human").expect("valid settings");

        logging.reload(&settings).expect("reload");
    }

    #[test]
    fn reload_switches_format_and_file() {
        let logging = Logging::init(&LogSettings::default()).expect("init");
        let path =
            std::env::temp_dir().join(format!("ticonnect-logging-test-{}.log", std::process::id()));
        let settings =
            LogSettings::new("info", Some(path.as_path()), "json").expect("valid settings");

        logging.reload(&settings).expect("reload format and file");

        assert!(path.exists(), "reload must create the log file");
        let _ = std::fs::remove_file(path);
    }
}
