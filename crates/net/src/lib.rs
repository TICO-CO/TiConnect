#![forbid(unsafe_code)]

//! LAN discovery and QUIC transport for TiConnect.

#[cfg(test)]
mod tests {
    #[test]
    fn links_core() {
        assert_eq!(ticonnect_core::version(), env!("CARGO_PKG_VERSION"));
    }
}
