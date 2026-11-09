use std::fs::{self, OpenOptions};
use std::io;
use std::path::Path;

use tracing_subscriber::{
    Layer,
    fmt::{self, writer::BoxMakeWriter},
};

use crate::config::LogFormat;
use crate::error::Result;
use crate::settings::{self, LogSettings};
use crate::types::OutputLayer;

pub fn make_output(settings: &LogSettings) -> Result<OutputLayer> {
    let writer = make_writer(settings::file(settings))?;

    let layer: OutputLayer = match settings::format(settings) {
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
