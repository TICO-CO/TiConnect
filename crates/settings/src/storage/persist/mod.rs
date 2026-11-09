mod stored_table;

use std::path::Path;

use super::Location;
use crate::error::Result;
use crate::settings::schema::SettingsValues;
use stored_table::StoredTable;

pub fn load_values(location: &Location) -> Result<SettingsValues> {
    StoredTable::read(location.file())?.values()
}

pub fn save_values(path: &Path, values: &SettingsValues) -> Result<()> {
    StoredTable::read(path)?.save(path, values)
}
