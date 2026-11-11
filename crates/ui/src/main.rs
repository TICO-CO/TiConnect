#![forbid(unsafe_code)]

use anyhow::Context;

fn main() -> anyhow::Result<()> {
    ticonnect_runtime::init().context("initialize runtime")?;
    ticonnect_ui::app::run().context("run TiConnect UI")?;
    Ok(())
}
