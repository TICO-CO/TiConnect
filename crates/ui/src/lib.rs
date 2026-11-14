#![forbid(unsafe_code)]

pub mod app;
pub mod autostart;
pub mod client;
pub mod error;
pub mod model;
pub mod views;

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
