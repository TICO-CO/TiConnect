use std::fs::{self, OpenOptions};
use std::io;
use std::path::Path;
use std::sync::Mutex;

use tracing::Subscriber;
use tracing_subscriber::{
    EnvFilter, Layer, Registry,
    fmt::{self, writer::BoxMakeWriter},
    layer::{Layered, SubscriberExt},
    registry::LookupSpan,
    reload,
    util::SubscriberInitExt,
};

use crate::config::{Config, LogFormat};
use crate::error::{Error, Result};

type FilterReload = reload::Layer<EnvFilter, Registry>;
type FilterHandle = reload::Handle<EnvFilter, Registry>;
type BaseSubscriber = Layered<FilterReload, Registry>;

#[derive(Clone)]
pub struct Logging {
    filter: FilterHandle,
}

static LOGGING: Mutex<Option<Logging>> = Mutex::new(None);

impl Logging {
    pub fn init(config: &Config) -> Result<Self> {
        let mut guard = LOGGING
            .lock()
            .map_err(|_| Error::Logging("logging initialization lock poisoned".to_owned()))?;

        if let Some(logging) = guard.as_ref() {
            return Ok(logging.clone());
        }

        let filter = EnvFilter::try_new(&config.log_level)
            .map_err(|error| Error::Logging(format!("invalid log filter: {error}")))?;
        let (filter_layer, filter_handle) = reload::Layer::new(filter);
        let output = make_output::<BaseSubscriber>(config)?;

        tracing_subscriber::registry()
            .with(filter_layer)
            .with(output)
            .try_init()
            .map_err(|error| Error::Logging(error.to_string()))?;

        let logging = Logging {
            filter: filter_handle,
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

    pub fn reload_filter(&self, config: &Config) -> Result<()> {
        let filter = EnvFilter::try_new(&config.log_level)
            .map_err(|error| Error::Logging(format!("invalid log filter: {error}")))?;

        self.filter
            .reload(filter)
            .map_err(|error| Error::Logging(error.to_string()))
    }
}

fn make_output<S>(config: &Config) -> Result<Box<dyn Layer<S> + Send + Sync + 'static>>
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    let writer = make_writer(config.log_file.as_deref())?;

    let layer: Box<dyn Layer<S> + Send + Sync + 'static> = match config.log_format {
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

    use super::Logging;
    use crate::config::Config;
    use crate::error::Error;

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
        let config = Config::default();

        let first = Logging::init(&config).expect("first init");
        let second = Logging::init(&config).expect("second init");

        assert!(Logging::get().is_some());
        first
            .reload_filter(&config)
            .expect("reload after second init");
        second
            .reload_filter(&config)
            .expect("reload after second init");
    }

    #[test]
    fn reload_filter_accepts_valid_config() {
        let logging = Logging::init(&Config::default()).expect("init");
        let config = Config {
            log_level: "warn".to_owned(),
            ..Config::default()
        };

        logging.reload_filter(&config).expect("reload");
    }

    #[test]
    fn reload_filter_rejects_invalid_filter() {
        let logging = Logging::init(&Config::default()).expect("init");
        let config = Config {
            log_level: "[".to_owned(),
            ..Config::default()
        };

        let error = logging
            .reload_filter(&config)
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
