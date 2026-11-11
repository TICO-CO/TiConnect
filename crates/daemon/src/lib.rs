#![forbid(unsafe_code)]

pub mod config_watch;
pub mod identity;
pub mod runtime;
pub mod service;
pub mod storage;

#[cfg(test)]
mod tests {
    #[test]
    fn links_core() {
        assert_eq!(ticonnect_core::version(), env!("CARGO_PKG_VERSION"));
    }
}
