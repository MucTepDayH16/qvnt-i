#![allow(clippy::enum_variant_names)]

mod cli;
mod int_tree;
mod lines;
mod process;
mod program;
mod utils;

fn main() -> program::ProgramResult {
    let cli = cli::CliArgs::new()?;
    program::Program::new(cli)?.run()?;
    Ok(())
}
