#![forbid(unsafe_code)]

mod config;
mod error;
mod logging;
mod output;
mod settings;
mod types;

pub use config::{LogFormat, LogLevel};
pub use error::{Error, Result};
pub use logging::Logging;
pub use settings::LogSettings;
