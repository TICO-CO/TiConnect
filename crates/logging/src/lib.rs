#![forbid(unsafe_code)]

//! Tracing setup for TiConnect.

use std::fs::{self, OpenOptions};
use std::io;
use std::path::Path;
use std::sync::Mutex;

mod config;
mod error;

pub use config::{LogFormat, LogSettings};
pub use error::{Error, Result};
use tracing_subscriber::{
    EnvFilter, Layer, Registry,
    fmt::{self, writer::BoxMakeWriter},
    layer::{Layered, SubscriberExt},
    reload,
    util::SubscriberInitExt,
};

type FilterReload = reload::Layer<EnvFilter, Registry>;
type FilterHandle = reload::Handle<EnvFilter, Registry>;
type BaseSubscriber = Layered<FilterReload, Registry>;

type OutputLayer = Box<dyn Layer<BaseSubscriber> + Send + Sync + 'static>;
type OutputHandle = reload::Handle<OutputLayer, BaseSubscriber>;

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

        let filter = EnvFilter::try_new(&settings.level)
            .map_err(|error| Error::Logging(format!("invalid log filter: {error}")))?;
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
        let filter = EnvFilter::try_new(&settings.level)
            .map_err(|error| Error::Logging(format!("invalid log filter: {error}")))?;
        let output = make_output(settings)?;

        self.output
            .reload(output)
            .map_err(|error| Error::Logging(error.to_string()))?;
        self.filter
            .reload(filter)
            .map_err(|error| Error::Logging(error.to_string()))
    }
}

fn make_output(
    settings: &LogSettings,
) -> Result<Box<dyn Layer<BaseSubscriber> + Send + Sync + 'static>> {
    let writer = make_writer(settings.file.as_deref())?;

    let layer: Box<dyn Layer<BaseSubscriber> + Send + Sync + 'static> = match settings.format {
        LogFormat::Human => fmt::layer().with_writer(writer).boxed(),
        LogFormat::Json => fmt::layer().with_writer(writer).json().boxed(),
    };

    Ok(layer)
}

fn make_writer(path: Option<&Path>) -> Result<BoxMakeWriter> {
    match path {
        Some(path) => {
            if let Some(parent) = path.parent()
                && !parent.as_os_str().is_empty()
            {
                fs::create_dir_all(parent)?;
            }

            let file = OpenOptions::new().create(true).append(true).open(path)?;
            Ok(BoxMakeWriter::new(file))
        }
        None => Ok(BoxMakeWriter::new(io::stdout)),
    }
}

#[cfg(test)]
mod tests {
    use std::io::{self, Write};
    use std::sync::{Arc, Mutex};

    use tracing_subscriber::fmt::writer::MakeWriter;

    use super::{Error, LogFormat, LogSettings, Logging};

    #[derive(Clone)]
    struct Buffer(Arc<Mutex<Vec<u8>>>);

    impl<'a> MakeWriter<'a> for Buffer {
        type Writer = BufferWriter;

        fn make_writer(&'a self) -> Self::Writer {
            BufferWriter(self.0.clone())
        }
    }

    struct BufferWriter(Arc<Mutex<Vec<u8>>>);

    impl Write for BufferWriter {
        fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
            self.0
                .lock()
                .expect("buffer lock")
                .extend_from_slice(buffer);
            Ok(buffer.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

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
        let settings = LogSettings {
            level: "warn".to_owned(),
            ..LogSettings::default()
        };

        logging.reload(&settings).expect("reload");
    }

    #[test]
    fn reload_switches_format_and_file() {
        let logging = Logging::init(&LogSettings::default()).expect("init");
        let path =
            std::env::temp_dir().join(format!("ticonnect-logging-test-{}.log", std::process::id()));
        let settings = LogSettings {
            format: LogFormat::Json,
            file: Some(path.clone()),
            ..LogSettings::default()
        };

        logging.reload(&settings).expect("reload format and file");

        assert!(path.exists(), "reload must create the log file");
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn reload_rejects_invalid_filter() {
        let logging = Logging::init(&LogSettings::default()).expect("init");
        let settings = LogSettings {
            level: "[".to_owned(),
            ..LogSettings::default()
        };

        let error = logging
            .reload(&settings)
            .expect_err("invalid filter must fail");

        assert!(matches!(error, Error::Logging(_)));
    }

    #[test]
    fn emits_log_line_at_configured_level() {
        let buffer = Arc::new(Mutex::new(Vec::new()));
        let subscriber = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::INFO)
            .with_writer(Buffer(buffer.clone()))
            .finish();

        tracing::subscriber::with_default(subscriber, || {
            tracing::info!("foundation smoke test");
        });

        let output = String::from_utf8(buffer.lock().expect("buffer lock").clone())
            .expect("log output must be UTF-8");
        assert!(output.contains("foundation smoke test"));
    }
}
