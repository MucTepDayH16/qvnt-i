mod cli;
mod int_tree;
mod lines;
mod process;
mod program;
mod utils;

fn main() -> program::ProgramResult<()> {
    program::Program::new()?.run()?;
    Ok(())
}
