#![forbid(unsafe_code)]

//! Persisted and runtime settings for TiConnect.

#[macro_use]
mod settings_macro;

mod error;
mod log;
mod overrides;
mod schema;
mod settings;
mod storage;

pub use error::{Error, Result};
pub use log::{LogFormat, LogSettings};
pub use settings::Settings;
