use std::path::PathBuf;

#[derive(clap::Parser, Debug)]
#[clap(name = "QVNT Interpreter", author, version, about, long_about = None)]
pub struct CliArgs {
    #[clap(index(1), help = "OpenQASM input files")]
    pub inputs: Vec<PathBuf>,
    
    #[clap(short = 'H', long, help = "History path for interpreter commands")]
    pub history: Option<PathBuf>,

    #[cfg(feature = "tracing")]
    #[clap(short = 'l', long = "logs", help = "Logs file path")]
    pub logs_enabled: Option<PathBuf>,
}

impl CliArgs {
    pub fn new() -> Self {
        <Self as clap::Parser>::parse()
    }
}
