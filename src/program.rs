use std::fmt;

use qvnt::prelude::Int;
use rustyline::{error::ReadlineError, Editor};

use crate::{
    cli::CliArgs,
    int_tree::IntTree,
    process::{self, Process},
};

pub fn leak_string<'t>(s: String) -> &'t str {
    let s = Box::leak(s.into_boxed_str()) as &'t str;
    // eprintln!("Leakage {{ ptr: {:?}, len: {} }}", s as *const _, s.len());
    s
}

pub const ROOT_TAG: &str = ".";

#[derive(Debug)]
pub enum ProgramError {
    ProcessError(process::Error),
    ReadlineError(ReadlineError),
    ClapError(clap::Error),
}

impl fmt::Display for ProgramError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProgramError::ProcessError(err) => err.fmt(f),
            ProgramError::ReadlineError(err) => err.fmt(f),
            ProgramError::ClapError(err) => err.fmt(f),
        }
    }
}

pub type ProgramResult<T = ()> = std::result::Result<T, ProgramError>;

pub(crate) struct Program<'t> {
    pub dbg: bool,
    pub interact: Editor<()>,
    pub curr_process: Process<'t>,
    pub int_tree: IntTree<'t>,
}

fn handle_error(result: process::Result, dbg: bool) -> Option<ProgramResult> {
    use process::Error;

    match result {
        Ok(()) => None,
        Err(err @ (Error::Inner | Error::Unimplemented)) => {
            eprintln!("Internal Error: Please report this to the developer.");
            Some(Err(ProgramError::ProcessError(err)))
        }
        Err(Error::Dyn(err)) => {
            if dbg {
                eprintln!("{:?}\n", err);
            } else {
                eprintln!("{}\n", err);
            }
            None
        }
        Err(Error::Quit) => Some(Ok(())),
    }
}

impl<'t> Program<'t> {
    pub fn new(cli: CliArgs) -> ProgramResult<Self> {
        const PROLOGUE: &str = "QVNT - Interactive QASM Interpreter\n\n";
        print!("{}", PROLOGUE);

        let mut interact = Editor::new();
        let _ = interact.load_history(&cli.history);

        let mut new = Self {
            dbg: cli.dbg,
            interact,
            curr_process: Process::new(Int::default()),
            int_tree: IntTree::with_root(ROOT_TAG),
        };

        if let Some(path) = cli.input {
            if let Some(result) = handle_error(
                new.curr_process.load_qasm(&mut new.int_tree, path.into()),
                new.dbg,
            ) {
                result?;
            }
        }

        Ok(new)
    }

    pub fn run(mut self) -> ProgramResult {
        const SIGN: &str = "|Q> ";
        const BLCK: &str = "... ";

        let mut block = (false, String::new());
        let ret_code = loop {
            match self.interact.readline(if block.0 { BLCK } else { SIGN }) {
                Ok(line) => {
                    println!();
                    self.interact.add_history_entry(&line);
                    match line.chars().last() {
                        Some('{') => {
                            block.1 += &line;
                            block.0 = true;
                        }
                        Some('}') if block.0 => {
                            block.1 += &line;
                            block.0 = false;
                            let line = leak_string(std::mem::take(&mut block.1));
                            if let Some(result) =
                                handle_error(self.curr_process.process_qasm(line), self.dbg)
                            {
                                break result;
                            }
                        }
                        _ if block.0 => {
                            block.1 += &line;
                        }
                        _ => {
                            if let Some(result) = handle_error(
                                self.curr_process.process(&mut self.int_tree, line),
                                self.dbg,
                            ) {
                                break result;
                            }
                        }
                    }
                }
                Err(err) => {
                    eprintln!("\nError: {:?}", err);
                    break Err(ProgramError::ReadlineError(err));
                }
            }
        };

        let _ = self.interact.save_history(".history");
        ret_code
    }
}

#[cfg(test)]
mod tests {
    use qvnt::prelude::Int;

    use crate::{int_tree::IntTree, process::*, program::leak_string};

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
                    let line = leak_string(block.1);
                    curr_process.process_qasm(line).unwrap();
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
