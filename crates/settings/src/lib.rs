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
pub use settings::schema::{
    AUDIO_VOLUME_MAX_PERCENT, AUDIO_VOLUME_MIN_PERCENT, Appearance, DEFAULT_AUDIO_VOLUME_PERCENT,
    SETTING_KEYS, SettingValue,
};
