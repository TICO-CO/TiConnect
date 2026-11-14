#![forbid(unsafe_code)]

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    ticonnect_cli::entry::run(std::env::args_os()).await
}
