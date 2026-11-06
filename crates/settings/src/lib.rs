#![forbid(unsafe_code)]

//! Persisted and effective settings for TiConnect.

mod api;
mod error;
mod overrides;
mod state;
mod storage;

#[cfg(test)]
#[allow(clippy::result_large_err)]
mod tests;

pub use api::Settings;
pub use error::{Error, Result};
