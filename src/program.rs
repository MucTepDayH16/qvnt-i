use qvnt::prelude::Int;
use rustyline::{error::ReadlineError, Config, Editor};

use crate::{
    cli::CliArgs,
    int_tree::IntTree,
    process::{self, Process},
};

pub const ROOT_TAG: &str = ".";

pub(crate) struct Program<'t> {
    pub history: String,
    pub dbg: bool,
    pub input: Option<String>,
    pub interact: Editor<()>,
    pub curr_process: Process<'t>,
    pub int_tree: IntTree<'t>,
}

fn decorate(result: process::Result<bool>, dbg: bool) -> Option<anyhow::Result<()>> {
    use process::Error;

    match result {
        Ok(true) => None,
        Ok(false) => Some(Ok(())),
        Err(err @ (Error::Inner | Error::Unimplemented)) => {
            eprintln!("Internal Error: Please report this to the developer.");
            Some(Err(anyhow::anyhow!(err)))
        }
        Err(err) => {
            if dbg {
                eprintln!("{:?}\n", err);
            } else {
                eprintln!("{}\n", err);
            }
            None
        }
    }
}

fn decorate_readline_error(err: ReadlineError, dbg: bool) -> Option<anyhow::Error> {
    Some(anyhow::anyhow!(match err {
        ReadlineError::Interrupted => {
            println!();
            return None
        },
        #[cfg(unix)]
        err @ ReadlineError::Errno(_) => err,
        #[cfg(windows)]
        err @ ReadlineError::SystemError(_) => err,
        err => {
            if dbg {
                eprintln!("{:?}\n", err);
            } else {
                eprintln!("{}\n", err);
            }
            return None;
        }
    }))
}

impl<'t> Program<'t> {
    pub fn new(cli: CliArgs) -> Self {
        const PROLOGUE: &str = "QVNT - Interactive QASM Interpreter\n\n";
        print!("{}", PROLOGUE);

        let config = Config::builder()
            .max_history_size(1_000)
            .check_cursor_position(true)
            .build();

        Self {
            history: cli.history,
            dbg: cli.dbg,
            input: cli.input,
            interact: Editor::with_config(config),
            curr_process: Process::new(Int::default()),
            int_tree: IntTree::with_root(ROOT_TAG),
        }
    }

    fn loop_fn(&mut self) -> anyhow::Result<()> {
        if let Some(path) = self.input.take() {
            if let Some(result) = decorate(
                self.curr_process
                    .load_qasm(&mut self.int_tree, path.into())
                    .map(|_| true),
                self.dbg,
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
                            let line = std::mem::take(&mut block.1);
                            if let Some(result) = decorate(
                                self.curr_process.process_qasm(line).map(|_| true),
                                self.dbg,
                            ) {
                                return result;
                            }
                        }
                        _ if block.0 => {
                            block.1 += &line;
                        }
                        _ => {
                            if let Some(result) = decorate(
                                self.curr_process.process(&mut self.int_tree, line),
                                self.dbg,
                            ) {
                                return result;
                            }
                        }
                    }
                }
                Err(err) => {
                    if let Some(err) = decorate_readline_error(err, self.dbg) {
                        return Err(err);
                    }
                }
            }
        }
    }

    pub fn run(mut self) -> anyhow::Result<()> {
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
