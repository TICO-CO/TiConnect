#![forbid(unsafe_code)]

//! Persisted and runtime settings for TiConnect.

mod config;
mod error;
mod log;
mod settings;

pub use error::{Error, Result};
pub use log::{LogFormat, LogSettings};
pub use settings::Settings;
