#![forbid(unsafe_code)]

//! Shared protocol, identity, pairing, and crypto types for TiConnect.

/// Return the TiConnect core crate version.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    #[test]
    fn version_matches_package_metadata() {
        assert_eq!(super::version(), env!("CARGO_PKG_VERSION"));
    }
}
