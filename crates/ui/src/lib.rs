#![forbid(unsafe_code)]

pub mod activation;
pub mod app;
pub mod autostart;
pub mod avatar;
pub mod bridge;
pub mod client;
pub mod error;
pub mod model;

use anyhow::Context;

pub fn run() -> anyhow::Result<()> {
    ticonnect_runtime::init().context("initialize runtime")?;
    app::run().context("run TiConnect UI")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn links_core() {
        assert_eq!(ticonnect_core::version(), env!("CARGO_PKG_VERSION"));
    }
}
