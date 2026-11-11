#![forbid(unsafe_code)]

pub mod fingerprint;
pub mod id;
pub mod identity;
pub mod ipc;

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
