mod cli;
mod int_tree;
mod lines;
mod process;
mod program;
mod utils;

fn main() -> anyhow::Result<()> {
    let cli = cli::CliArgs::new()?;
    program::Program::new(cli)
        .run()
        .map_err(|err| anyhow::anyhow!(err))?;
    Ok(())
}
