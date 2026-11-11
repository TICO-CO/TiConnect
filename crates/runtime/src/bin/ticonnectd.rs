#![forbid(unsafe_code)]

use anyhow::Context;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    ticonnect_runtime::run_daemon()
        .await
        .context("daemon failed")
}
