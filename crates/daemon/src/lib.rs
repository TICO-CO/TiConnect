#![forbid(unsafe_code)]

pub mod config_watch;
pub mod discovery_service;
pub mod identity;
pub mod messaging;
pub mod pairing_service;
pub mod reconnect_service;
pub mod runtime;
pub mod service;
pub mod storage;
pub mod tray;
pub mod trust;

#[cfg(test)]
mod tests {
    #[test]
    fn links_core() {
        assert_eq!(ticonnect_core::version(), env!("CARGO_PKG_VERSION"));
    }
}
