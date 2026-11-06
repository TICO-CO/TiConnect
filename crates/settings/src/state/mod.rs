#[macro_use]
mod macros;

mod schema;

use std::path::Path;

use self::schema::SettingsChanges;
use crate::error::Result;
use crate::overrides;
use crate::storage::Storage;

pub(crate) use self::schema::SettingsValues;

#[derive(Debug, Clone)]
pub(crate) struct SettingsState {
    storage: Storage,
    active: SettingsValues,
    changes: SettingsChanges,
}

impl SettingsState {
    pub(crate) fn load() -> Result<Self> {
        Self::from_storage(Storage::open_default()?)
    }

    pub(crate) fn load_from(path: &Path) -> Result<Self> {
        Self::from_storage(Storage::at(path)?)
    }

    pub(crate) fn save(&mut self) -> Result<()> {
        if self.changes.is_empty() {
            return Ok(());
        }

        let mut next = self.storage.saved().clone();
        self.changes.apply(&self.active, &mut next);
        self.storage.save(&next)?;
        self.changes.clear();
        Ok(())
    }

    pub(crate) fn reset(&mut self) -> Result<()> {
        let active = overrides::load(&SettingsValues::default())?;

        self.storage.reset()?;
        self.active = active;
        self.changes.clear();
        Ok(())
    }

    fn from_storage(storage: Storage) -> Result<Self> {
        let active = overrides::load(storage.saved())?;

        Ok(Self {
            storage,
            active,
            changes: SettingsChanges::default(),
        })
    }
}
