use std::{fmt, path::PathBuf};

use qvnt::prelude::Int;
use rustyline::{error::ReadlineError, Config, Editor};

use crate::{
    cli::CliArgs,
    int_tree::IntTree,
    process::{self, Process},
};

pub const ROOT_TAG: &str = ".";

pub type ProgramResult<T = ()> = Result<T, ProgramError>;

#[derive(Debug)]
pub enum ProgramError {
    HistoryPath,
    Process(process::Error),
    Readline(ReadlineError),
}

impl From<process::Error> for ProgramError {
    fn from(err: process::Error) -> Self {
        Self::Process(err)
    }
}

impl From<ReadlineError> for ProgramError {
    fn from(err: ReadlineError) -> Self {
        Self::Readline(err)
    }
}

impl From<ProgramError> for anyhow::Error {
    fn from(err: ProgramError) -> Self {
        anyhow::anyhow!(err)
    }
}

impl fmt::Display for ProgramError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProgramError::HistoryPath => write!(f, "Cannot find HOME or CWD"),
            ProgramError::Process(err) => write!(f, "Process error: {}", err),
            ProgramError::Readline(err) => write!(f, "Readline error: {}", err),
        }
    }
}

impl ProgramError {
    pub fn should_echo(&self) -> bool {
        match self {
            ProgramError::HistoryPath => false,
            ProgramError::Process(err) => err.should_echo(),
            ProgramError::Readline(err) => !matches!(err, ReadlineError::Interrupted),
        }
    }

    pub fn is_fatal(&self) -> bool {
        match self {
            ProgramError::Process(process::Error::Inner | process::Error::Unimplemented) => true,
            #[cfg(unix)]
            ProgramError::Readline(ReadlineError::Errno(_)) => true,
            #[cfg(windows)]
            ProgramError::Readline(ReadlineError::SystemError(_)) => true,
            _ => false,
        }
    }
}

pub struct Program<'t> {
    pub history: PathBuf,
    pub input: Option<PathBuf>,
    pub interact: Editor<()>,
    pub curr_process: Process<'t>,
    pub int_tree: IntTree<'t>,
}

fn decorate<E: Into<ProgramError>>(result: Result<bool, E>) -> Option<ProgramResult<()>> {
    match result.map_err(Into::into) {
        Ok(true) => None,
        Ok(false) => Some(Ok(())),
        Err(err) if err.is_fatal() => Some(Err(err)),
        Err(err) => {
            if cfg!(debug_assertions) {
                eprintln!("{:?}", err);
            } else if err.should_echo() {
                eprintln!("{}", err);
            }
            None
        }
    }
}

impl<'t> Program<'t> {
    pub fn new(cli: CliArgs) -> ProgramResult<Self> {
        let history = if let Some(history) = cli.history {
            if !history.is_file() {
                return Err(ProgramError::HistoryPath);
            }
            history
        } else {
            let mut history_path = home::home_dir()
                .or_else(|| std::env::current_dir().ok())
                .filter(|p| p.is_dir())
                .ok_or(ProgramError::HistoryPath)?;
            history_path.push(".qvnt_history");
            history_path
        };

        let config = Config::builder()
            .max_history_size(1_000)
            .check_cursor_position(true)
            .build();

        Ok(Self {
            history,
            input: cli.input,
            interact: Editor::with_config(config),
            curr_process: Process::new(Int::default()),
            int_tree: IntTree::with_root(ROOT_TAG),
        })
    }

    fn loop_fn(&mut self) -> ProgramResult<()> {
        if let Some(path) = self.input.take() {
            if let Some(result) = decorate(
                self.curr_process
                    .load_qasm(&mut self.int_tree, path)
                    .map(|_| true),
            ) {
                result?;
            }
        }

        const SIGN: &str = "|Q> ";
        const BLCK: &str = "... ";

        let mut block = (false, String::new());
        loop {
            match self.interact.readline(if block.0 { BLCK } else { SIGN }) {
                Ok(line) => {
                    self.interact.add_history_entry(&line);
                    match line.chars().last() {
                        Some('{') => {
                            block.1 += &line;
                            block.0 = true;
                        }
                        Some('}') if block.0 => {
                            block.1 += &line;
                            block.0 = false;
                            let line = std::mem::take(&mut block.1);
                            if let Some(result) =
                                decorate(self.curr_process.process_qasm(line).map(|_| true))
                            {
                                return result;
                            }
                        }
                        _ if block.0 => {
                            block.1 += &line;
                        }
                        _ => {
                            if let Some(result) =
                                decorate(self.curr_process.process(&mut self.int_tree, line))
                            {
                                return result;
                            }
                        }
                    }
                }
                Err(err) => {
                    let err = Err(ProgramError::Readline(err));
                    if let Some(err) = decorate(err) {
                        return err;
                    }
                }
            }
        }
    }

    pub fn run(mut self) -> ProgramResult<()> {
        const PROLOGUE: &str = "QVNT - Interactive QASM Interpreter\n\n";
        print!("{}", PROLOGUE);

        let _ = self.interact.load_history(&self.history);
        let ret_code = self.loop_fn();
        let _ = self.interact.save_history(&self.history);

        ret_code
    }
}

#[cfg(test)]
mod tests {
    use qvnt::prelude::Int;

    use crate::{int_tree::IntTree, process::*};

    #[test]
    fn main_loop() {
        let mut int_tree = IntTree::with_root(crate::program::ROOT_TAG);
        let mut curr_process = Process::new(Int::default());
        let mut block = (false, String::new());

        let input = vec![
            (":tag ls", "Int { m_op: Set, q_reg: [], c_reg: [], q_ops: [], macros: {}, .. }"),
            ("qreg q[4];", "Int { m_op: Set, q_reg: [\"q\", \"q\", \"q\", \"q\"], c_reg: [], q_ops: [], macros: {}, .. }"),
            (":tag mk reg", "Int { m_op: Set, q_reg: [\"q\", \"q\", \"q\", \"q\"], c_reg: [], q_ops: [], macros: {}, .. }"),
            ("h q[2];", "Int { m_op: Set, q_reg: [\"q\", \"q\", \"q\", \"q\"], c_reg: [], q_ops: [H4], macros: {}, .. }"),
            (":tag mk ops", "Int { m_op: Set, q_reg: [\"q\", \"q\", \"q\", \"q\"], c_reg: [], q_ops: [H4], macros: {}, .. }"),
            (":tag ch reg", "Int { m_op: Set, q_reg: [\"q\", \"q\", \"q\", \"q\"], c_reg: [], q_ops: [], macros: {}, .. }"),
            (":tag root", "Int { m_op: Set, q_reg: [], c_reg: [], q_ops: [], macros: {}, .. }"),
            ("gate OOO(a, b) x, y { h x; rx(a+b) y; }", "Int { m_op: Set, q_reg: [], c_reg: [], q_ops: [], macros: {\"OOO\": Macro { regs: [\"x\", \"y\"], args: [\"a\", \"b\"], nodes: [(\"h\", [Register(\"x\")], []), (\"rx\", [Register(\"y\")], [\"a+b\"])] }}, .. }"),
            (":tag mk macro", "Int { m_op: Set, q_reg: [], c_reg: [], q_ops: [], macros: {\"OOO\": Macro { regs: [\"x\", \"y\"], args: [\"a\", \"b\"], nodes: [(\"h\", [Register(\"x\")], []), (\"rx\", [Register(\"y\")], [\"a+b\"])] }}, .. }"),
        ];

        for (line, expected_int) in input {
            let line = line.to_string();
            match line.chars().last() {
                Some('{') => {
                    block.1 += &line;
                    block.0 = true;
                }
                Some('}') if block.0 => {
                    block.1 += &line;
                    block.0 = false;
                    curr_process.process_qasm(block.1).unwrap();
                    block.1 = String::new();
                }
                _ if block.0 => {
                    block.1 += &line;
                }
                _ => {
                    curr_process.process(&mut int_tree, line).unwrap();
                }
            }

            assert_eq!(
                format!("{:?}", curr_process.int()),
                expected_int.to_string()
            );
        }
    }
}
