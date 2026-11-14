#![forbid(unsafe_code)]

pub mod discovery;
pub mod pairing;
pub mod tls;
pub mod transfer;
pub mod transport;

#[cfg(test)]
mod tests {
    #[test]
    fn links_core() {
        assert_eq!(ticonnect_core::version(), env!("CARGO_PKG_VERSION"));
    }
}
