mod stored_table;

use std::path::Path;

use super::Location;
use crate::error::Result;
use crate::settings::schema::{SettingsChanges, SettingsValues};
use stored_table::StoredTable;

pub fn load_values(location: &Location) -> Result<SettingsValues> {
    StoredTable::read(location.file())?.values()
}

pub fn update_values(
    path: &Path,
    active: &SettingsValues,
    changes: &SettingsChanges,
) -> Result<SettingsValues> {
    let table = StoredTable::read(path)?;
    let mut values = table.values()?;
    changes.apply(active, &mut values);
    table.save(path, &values)?;

    Ok(values)
}

#[cfg(test)]
pub fn save_values(path: &Path, values: &SettingsValues) -> Result<()> {
    StoredTable::read(path)?.save(path, values)
}
