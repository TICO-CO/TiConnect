mod load;
mod model;
mod output;
mod render;
mod validate;

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunMode {
    Write,
    Check,
}

pub fn run(mode: RunMode) -> Result<()> {
    let root = root()?;
    let model = load::load(&root)?;
    validate::validate(&root, &model)?;
    let outputs = render::render_all(&model);

    match mode {
        RunMode::Write => {
            output::write(&root, &outputs)?;
            println!("generated {} architecture documents", outputs.len());
        }
        RunMode::Check => {
            output::check(&root, &outputs)?;
            println!("architecture graph check passed");
        }
    }
    Ok(())
}

fn root() -> Result<PathBuf> {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest
        .parent()
        .map(Path::to_path_buf)
        .context("xtask has no parent directory")
}
