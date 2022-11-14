use std::{collections::HashMap, fmt, path::PathBuf};

use qvnt::qasm::{Ast, Int, Sym};

use crate::{
    int_tree::IntTree,
    lines::{self, Command, Line},
    utils::{drop_leakage::leak_string, owned_errors, owned_errors::ToOwnedError},
};

#[derive(Debug)]
pub enum Error {
    Io(std::io::Error),
    Lines(lines::Error),
    Int(owned_errors::int::OwnedError),
    Ast(owned_errors::ast::OwnedError),
    Inner,
    #[allow(dead_code)]
    Unimplemented,
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err)
    }
}

impl From<lines::Error> for Error {
    fn from(err: lines::Error) -> Self {
        Self::Lines(err)
    }
}

impl<'t> From<qvnt::qasm::int::Error<'t>> for Error {
    fn from(err: qvnt::qasm::int::Error<'t>) -> Self {
        Self::Int(err.own())
    }
}

impl<'t> From<qvnt::qasm::ast::Error<'t>> for Error {
    fn from(err: qvnt::qasm::ast::Error<'t>) -> Self {
        Self::Ast(err.own())
    }
}

const ON_UNEXPECTED: &str = "This error should never occur, contact developpers and provide the way to reproduce this error";

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io(err) => write!(f, "{}", err),
            Error::Lines(err) => write!(f, "{}", err),
            Error::Int(err) => write!(f, "{}", err),
            Error::Ast(err) => write!(f, "{}", err),
            Error::Inner => write!(f, "Inner functional error. {}", ON_UNEXPECTED),
            Error::Unimplemented => write!(f, "Unimplemented function. {}", ON_UNEXPECTED),
        }
    }
}

impl std::error::Error for Error {}

// impl<E: std::error::Error + 'static> From<E> for Error {
//     fn from(e: E) -> Self {
//         Self::Dyn(e.into())
//     }
// }

pub type Result<T = ()> = std::result::Result<T, Error>;

pub struct Process<'t> {
    head: Int<'t>,
    int: Int<'t>,
    sym: Sym,
    storage: HashMap<PathBuf, Ast<'t>>,
}

impl<'t> Process<'t> {
    pub fn new(int: Int<'t>) -> Self {
        Self {
            head: Int::default(),
            int: int.clone(),
            sym: Sym::new(int),
            storage: HashMap::new(),
        }
    }

    pub fn int(&self) -> Int<'t> {
        let int = self.int.clone();
        unsafe { int.append_int(self.head.clone()) }
    }

    fn reset(&mut self, int: Int<'t>) {
        self.head = Int::default();
        self.int = int;
    }

    fn sym_update(&mut self) {
        let int = self.int();
        self.sym.init(int);
    }

    fn sym_go(&mut self) {
        self.sym_update();
        self.sym.reset();
        self.sym.finish();
    }

    pub fn process(&mut self, int_set: &mut IntTree<'t>, line: String) -> Result<bool> {
        match line.parse::<Line>()? {
            Line::Qasm => self.process_qasm(line).map(|_| true),
            Line::Commands(cmds) => self.process_cmd(int_set, cmds.into_iter()),
        }
    }

    pub fn process_qasm(&mut self, line: String) -> Result {
        let line = leak_string(line);
        let ast = Ast::from_source(line)?;
        self.int.ast_changes(&mut self.head, ast)?;
        Ok(())
    }

    pub fn process_cmd(
        &mut self,
        int_tree: &mut IntTree<'t>,
        mut cmds: impl Iterator<Item = Command> + Clone,
    ) -> Result<bool> {
        while let Some(cmd) = cmds.next() {
            match cmd {
                Command::Loop(n) => {
                    for _ in 0..n {
                        self.process_cmd(int_tree, cmds.clone())?;
                    }
                    break;
                }
                Command::Tags(tag_cmd) => self.process_tag_cmd(int_tree, tag_cmd)?,
                Command::Go => self.sym_go(),
                Command::Load(path) => self.load_qasm(int_tree, path)?,
                Command::Class => {
                    self.sym_update();
                    println!("CReg: {}\n", self.sym.get_class().get())
                }
                Command::Polar => {
                    self.sym_update();
                    println!("QReg polar: {:.4?}\n", self.sym.get_polar_wavefunction());
                }
                Command::Probs => {
                    self.sym_update();
                    println!("QReg probabilities: {:.4?}\n", self.sym.get_probabilities());
                }
                Command::Ops => println!("Operations: {}\n", self.int().get_ops_tree()),
                Command::Names => {
                    println!(
                        "QReg: {}\nCReg: {}\n",
                        self.int().get_q_alias(),
                        self.int().get_c_alias()
                    );
                }
                Command::Help => println!("{}", lines::HELP),
                Command::Quit => return Ok(false),
            }
        }

        Ok(true)
    }

    pub fn process_tag_cmd(
        &mut self,
        int_tree: &mut IntTree<'t>,
        tag_cmd: crate::int_tree::Command,
    ) -> Result {
        use crate::int_tree::Command;
        match tag_cmd {
            Command::List => println!("{:?}\n", int_tree.keys()),
            Command::Create(tag) => {
                if !int_tree.commit(&tag, self.head.clone()) {
                    return Err(lines::Error::ExistedTagName(tag).into());
                } else {
                    unsafe {
                        self.int = self.int.clone().append_int(std::mem::take(&mut self.head))
                    };
                }
            }
            Command::Remove(tag) => {
                use crate::int_tree::RemoveStatus::*;
                match int_tree.remove(&tag) {
                    Removed => {}
                    NotFound => return Err(lines::Error::WrongTagName(tag).into()),
                    IsParent => return Err(lines::Error::TagIsParent(tag).into()),
                    IsHead => return Err(lines::Error::TagIsHead(tag).into()),
                }
            }
            Command::Checkout(tag) => {
                if !int_tree.checkout(&tag) {
                    return Err(lines::Error::WrongTagName(tag).into());
                } else {
                    let new_int = int_tree.collect_to_head().ok_or(Error::Inner)?;
                    self.reset(new_int);
                }
            }
            Command::Root => {
                if !int_tree.checkout(crate::program::ROOT_TAG) {
                    return Err(Error::Inner);
                } else {
                    self.reset(Int::default());
                }
            }
            Command::Help => println!("{}", crate::int_tree::HELP),
        }
        Ok(())
    }

    pub fn load_qasm(&mut self, int_tree: &mut IntTree<'t>, path: PathBuf) -> Result {
        let path_tag = format!("file://{}", path.display());
        if int_tree.checkout(&path_tag) {
            self.reset(int_tree.collect_to_head().ok_or(Error::Inner)?);
        } else {
            let default_ast = {
                let source = std::fs::read_to_string(&path)?;
                let source = leak_string(source);
                Ast::from_source(source)?
            };
            let ast = self.storage.entry(path).or_insert(default_ast).clone();
            int_tree.checkout("");
            let int = Int::new(ast)?;
            if !int_tree.commit(&path_tag, int.clone()) {
                return Err(Error::Inner);
            }
            self.reset(int);
        }
        Ok(())
    }
}
