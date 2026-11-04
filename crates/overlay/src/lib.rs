#![forbid(unsafe_code)]

//! TiConnect corner toast overlay agent.

#[cfg(test)]
mod tests {
    #[test]
    fn links_core() {
        assert_eq!(ticonnect_core::version(), env!("CARGO_PKG_VERSION"));
    }
}
