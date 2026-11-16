#![forbid(unsafe_code)]

fn main() -> anyhow::Result<()> {
    let action = ticonnect::parse(std::env::args_os().skip(1));
    match action {
        Ok(action) => ticonnect::run(action),
        Err(error) => {
            eprintln!("ticonnect: {error}");
            eprintln!();
            eprintln!("{}", ticonnect::HELP);
            std::process::exit(2);
        }
    }
}
