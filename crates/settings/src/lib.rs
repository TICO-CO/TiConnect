#![forbid(unsafe_code)]

mod error;
mod overrides;
mod settings;
mod storage;

#[cfg(test)]
#[allow(clippy::result_large_err)]
mod tests;

pub use error::{Error, Result};
pub use settings::Settings;
