#![forbid(unsafe_code)]

pub mod app;
pub mod autostart;
pub mod client;
pub mod error;
pub mod model;
pub mod views;

#[cfg(test)]
mod tests {
    #[test]
    fn links_core() {
        assert_eq!(ticonnect_core::version(), env!("CARGO_PKG_VERSION"));
    }
}
