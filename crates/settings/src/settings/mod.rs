#[macro_use]
mod macros;

pub mod schema;

use std::path::Path;

use self::schema::{SettingsChanges, SettingsValues};
use crate::error::Result;
use crate::overrides;
use crate::storage::Storage;

#[derive(Debug, Clone)]
pub struct Settings {
    storage: Storage,
    active: SettingsValues,
    changes: SettingsChanges,
}

impl Settings {
    pub fn load() -> Result<Self> {
        Self::from_storage(Storage::open()?)
    }

    pub fn load_from(path: impl AsRef<Path>) -> Result<Self> {
        Self::from_storage(Storage::open_at(path.as_ref().to_path_buf())?)
    }

    pub fn save(&mut self) -> Result<()> {
        if self.changes.is_empty() {
            return Ok(());
        }

        self.storage.update(&self.active, &self.changes)?;
        self.changes.clear();
        Ok(())
    }

    pub fn reset(&mut self) -> Result<()> {
        let active = overrides::load(&SettingsValues::default())?;

        self.storage.reset()?;
        self.active = active;
        self.changes.clear();
        Ok(())
    }

    fn from_storage(storage: Storage) -> Result<Self> {
        let active = overrides::load(storage.stored())?;

        Ok(Self {
            storage,
            active,
            changes: SettingsChanges::default(),
        })
    }
}
