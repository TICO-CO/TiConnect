use figment::{
    Figment,
    providers::{Env as FigmentEnv, Serialized},
};

use crate::error::{Error, Result};
use crate::schema::Values;

const ENV_PREFIX: &str = "TICONNECT_";

pub(crate) fn load(baseline: &Values) -> Result<Values> {
    Figment::new()
        .merge(Serialized::defaults(baseline.clone()))
        .merge(FigmentEnv::prefixed(ENV_PREFIX))
        .extract()
        .map_err(|error| Error::Settings(error.to_string()))
}
