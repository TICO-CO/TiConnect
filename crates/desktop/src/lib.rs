#![forbid(unsafe_code)]

pub mod capture;
pub mod input;
pub mod portal;

pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

pub fn core_version() -> &'static str {
    ticonnect_core::version()
}

#[cfg(test)]
mod tests {
    #[test]
    fn links_core() {
        assert_eq!(ticonnect_core::version(), env!("CARGO_PKG_VERSION"));
    }
}
