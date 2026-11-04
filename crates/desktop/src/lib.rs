#![forbid(unsafe_code)]

//! Desktop capture and input integration for TiConnect.

#[cfg(test)]
mod tests {
    #[test]
    fn links_core() {
        assert_eq!(ticonnect_core::version(), env!("CARGO_PKG_VERSION"));
    }
}
