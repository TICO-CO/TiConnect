mod location;
mod store;

#[cfg(test)]
#[allow(clippy::result_large_err)]
mod tests;

pub(crate) use location::default_data_dir;
pub(crate) use store::Storage;
