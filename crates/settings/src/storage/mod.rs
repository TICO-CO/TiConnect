mod io;
mod location;
mod lock;
mod persist;
mod store;

pub use location::Location;
pub use lock::acquire_write_lock;
#[cfg(test)]
pub use persist::save_values;
pub use persist::{load_values, update_values};
pub use store::Storage;

#[cfg(test)]
#[allow(clippy::result_large_err)]
mod tests;
