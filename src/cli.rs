use crate::program::{ProgramError, ProgramResult};

#[derive(clap::Parser, Debug)]
#[clap(name = "QVNT Interpreter", author, version, about, long_about = None)]
pub struct CliArgs {
    #[clap(short, long, help = "Specify QASM file path")]
    pub input: Option<String>,
    #[clap(long, help = "Set debug format for errors")]
    pub dbg: bool,
    #[clap(
        short,
        long,
        help = "Specify history path for interpreter commands",
        default_value_t = format!(
            "{}/.qvnt_history",
            std::env::var_os("HOME").and_then(|h| h.into_string().ok()).unwrap_or(".".into())
        )
    )]
    pub history: String,
}

impl CliArgs {
    pub fn new() -> ProgramResult<Self> {
        <Self as clap::StructOpt>::try_parse().map_err(ProgramError::Clap)
    }
}
