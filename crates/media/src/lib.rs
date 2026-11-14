#![forbid(unsafe_code)]

pub mod audio;
pub mod h264;
pub mod jitter;
pub mod notification;
pub mod opus_codec;
pub mod ringtone;

pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    #[test]
    fn links_core() {
        assert_eq!(ticonnect_core::version(), env!("CARGO_PKG_VERSION"));
    }
}
