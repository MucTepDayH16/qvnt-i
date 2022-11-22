use std::path::PathBuf;

#[derive(clap::Parser, Debug)]
#[clap(name = "QVNT Interpreter", author, version, about, long_about = None)]
pub struct CliArgs {
    #[clap(index(1), help = "OpenQASM file path")]
    pub input: Option<PathBuf>,
    #[clap(short = 'H', long, help = "History path for interpreter commands")]
    pub history: Option<PathBuf>,
}

impl CliArgs {
    pub fn new() -> Self {
        <Self as clap::Parser>::parse()
    }
}
