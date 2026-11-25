#![forbid(unsafe_code)]

pub mod activation;
pub mod app;
pub mod autostart;
pub mod avatar;
pub mod bridge;
pub mod client;
pub mod error;
pub mod model;
mod timing;

use anyhow::Context;

pub fn run() -> anyhow::Result<()> {
    timing::mark("ui run start");
    ticonnect_runtime::init().context("initialize runtime")?;
    timing::mark("runtime initialized");
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
