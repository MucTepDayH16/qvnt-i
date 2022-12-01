use std::{fmt, path::PathBuf};

use qvnt::prelude::Int;
use rustyline::{error::ReadlineError, Config, Editor};

use crate::{
    cli::CliArgs,
    int_tree::Tree,
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
    pub inputs: Vec<PathBuf>,
    pub interact: Editor<()>,
    pub curr_process: Process<'t>,
    pub int_tree: Tree<Int<'t>>,
}

impl<'t> Program<'t> {
    pub fn new() -> ProgramResult<Self> {
        let cli = CliArgs::new();

        #[cfg(feature = "tracing")]
        if let Some(logs_path) = cli.logs {
            let file = std::fs::File::create(logs_path).map_err(process::Error::Io)?;
            env_logger::Builder::default()
                .target(env_logger::Target::Pipe(Box::new(file)))
                .filter(None, log::LevelFilter::Trace)
                .filter(Some("rustyline"), log::LevelFilter::Info)
                .parse_default_env()
                .init();
        }

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
            .history_ignore_dups(true)
            .auto_add_history(true)
            .check_cursor_position(true)
            .build();

        Ok(Self {
            history,
            inputs: cli.inputs,
            interact: Editor::with_config(config)?,
            curr_process: Process::new(Int::default()),
            int_tree: Tree::with_root(ROOT_TAG),
        })
    }

    fn loop_fn(&mut self) -> ProgramResult<()> {
        for path in self.inputs.drain(..) {
            if let Some(result) = Self::decorate_error(
                self.curr_process
                    .load_qasm(&mut self.int_tree, path, false)
                    .map(|_| true),
            ) {
                result?;
            }
        }

        const SIGN: &str = "|Q> ";
        const BLCK: &str = "... ";

        let mut block = (false, String::new());
        loop {
            let maybe_result = match self.interact.readline(if block.0 { BLCK } else { SIGN }) {
                Ok(line) => self.process_line(&mut block, line),
                Err(err) => Self::decorate_error(Err(err)),
            };

            if let Some(result) = maybe_result {
                return result;
            }
        }
    }

    fn process_line(&mut self, block: &mut (bool, String), line: String) -> Option<ProgramResult> {
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
                    Self::decorate_error(self.curr_process.process_qasm(line).map(|_| true))
                {
                    return Some(result);
                }
            }
            _ if block.0 => {
                block.1 += &line;
            }
            _ => {
                if let Some(result) =
                    Self::decorate_error(self.curr_process.process(&mut self.int_tree, line))
                {
                    return Some(result);
                }
            }
        }

        None
    }

    fn decorate_error<E: Into<ProgramError>>(result: Result<bool, E>) -> Option<ProgramResult<()>> {
        let ret = match result.map_err(Into::into) {
            Ok(true) => None,
            Ok(false) => Some(Ok(())),
            Err(err) => {
                log::error!(target: "qvnt_i::main", "{:?}", err);
                if err.is_fatal() {
                    Some(Err(err))
                } else {
                    if cfg!(debug_assertions) {
                        eprintln!("{:?}", err);
                    } else if err.should_echo() {
                        eprintln!("{}", err);
                    }
                    None
                }
            }
        };

        ret
    }

    pub fn run(mut self) -> ProgramResult<()> {
        const PROLOGUE: &str = "QVNT - Interactive QASM Interpreter";
        print!("{}\n\n", PROLOGUE);

        if let Err(err) = self.interact.load_history(&self.history) {
            log::error!(target: "qvnt_i::main", "History not loaded: {}", err);
        }
        let ret_code = self.loop_fn();
        if let Err(err) = self.interact.save_history(&self.history) {
            log::error!(target: "qvnt_i::main", "History not saved: {}", err);
        }

        ret_code
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn main_loop() {
        let mut program = Program::new().unwrap();
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
            assert!(program.process_line(&mut block, line).is_none());

            assert_eq!(
                format!("{:?}", program.curr_process.int()),
                expected_int.to_string()
            );
        }
    }
}
