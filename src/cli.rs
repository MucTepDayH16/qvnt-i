use std::path::PathBuf;

#[derive(clap::Parser, Debug)]
#[clap(name = "QVNT Interpreter", author, version, about, long_about = None)]
pub struct CliArgs {
    #[clap(short, long, help = "Specify QASM file path")]
    pub input: Option<PathBuf>,
    #[clap(short, long, help = "Specify history path for interpreter commands")]
    pub history: Option<PathBuf>,
}

impl CliArgs {
    pub fn new() -> clap::Result<Self> {
        <Self as clap::StructOpt>::try_parse()
    }
}
