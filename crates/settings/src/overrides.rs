use figment::{
    Figment,
    providers::{Env as FigmentEnv, Serialized},
};

use crate::error::{Error, Result};
use crate::settings::schema::SettingsValues;

const ENV_PREFIX: &str = "TICONNECT_";

pub fn load(baseline: &SettingsValues) -> Result<SettingsValues> {
    Figment::new()
        .merge(Serialized::defaults(baseline.clone()))
        .merge(FigmentEnv::prefixed(ENV_PREFIX).split("__"))
        .extract()
        .map_err(|error| Error::Settings(error.to_string()))
}
