use std::{
    cell::RefCell,
    collections::HashMap,
    fmt,
    rc::{Rc, Weak},
};

use qvnt::qasm::Int;

use crate::utils;

#[derive(Debug)]
pub struct IntTree<'t> {
    head: RefCell<Option<Rc<String>>>,
    map: HashMap<Rc<String>, (Weak<String>, Int<'t>)>,
}

pub enum RemoveStatus {
    Removed,
    NotFound,
    IsParent,
    IsHead,
}

impl<'t> IntTree<'t>
where
    Self: 't,
{
    pub fn with_root<S: ToString>(root: S) -> Self {
        let root = Rc::new(root.to_string());
        let map = HashMap::from([(Rc::clone(&root), (Weak::new(), Int::default()))]);
        Self {
            head: RefCell::new(Some(root)),
            map,
        }
    }

    /// *TODO*: Display as a tree
    pub fn keys(&self) -> Vec<(Rc<String>, Rc<String>)> {
        self.map
            .iter()
            .filter_map(|(a, (b, _))| Some((Rc::clone(a), Weak::upgrade(b)?)))
            .collect()
    }

    pub fn commit<S: AsRef<str>>(&mut self, tag: S, change: Int<'t>) -> bool {
        let tag = tag.as_ref().to_string();

        if self.map.contains_key(&tag) {
            log::trace!(target: "qvnt_i::tag::commit", "Tag {} already exists", tag);
            return false;
        }

        let tag = Rc::new(tag);
        let old_head = match &*self.head.borrow() {
            Some(rc) => Rc::downgrade(rc),
            None => Weak::new(),
        };
        *self.head.borrow_mut() = Some(Rc::clone(&tag));
        log::trace!(target: "qvnt_i::tag::commit", "Tag {} created", tag);
        self.map.insert(tag, (old_head, change));

        true
    }

    pub fn checkout<S: AsRef<str>>(&self, tag: S) -> bool {
        let tag = tag.as_ref().to_string();

        match self.map.get_key_value(&tag) {
            Some(entry) => {
                *self.head.borrow_mut() = Some(Rc::clone(entry.0));
                log::trace!(target: "qvnt_i::tag::checkout", "New head is on tag {}", tag);
                true
            }
            None => {
                log::trace!(target: "qvnt_i::tag::checkout", "Tag {} doesn't exist", tag);
                false
            }
        }
    }

    pub fn collect_to_head(&self) -> Option<Int<'t>> {
        let mut start = Rc::clone(self.head.borrow().as_ref()?);
        let mut int_changes = Int::<'t>::default();

        log::trace!(target: "qvnt_i::tag::collect", "Staring collection");
        loop {
            log::trace!(target: "qvnt_i::tag::collect", "Collection step to tag {}", start);
            let curr = self.map.get(&start)?.clone();
            int_changes = unsafe { int_changes.prepend_int(curr.1.clone()) };
            if let Some(next) = Weak::upgrade(&curr.0) {
                start = Rc::clone(&next);
            } else {
                break Some(int_changes);
            }
        }
    }

    pub fn remove<S: AsRef<str>>(&mut self, tag: S) -> RemoveStatus {
        let tag = tag.as_ref().to_string();

        if self.head.borrow().as_deref() == Some(&tag) {
            return RemoveStatus::IsHead;
        }

        for tags in self.map.iter() {
            if let Some(par_tag) = Weak::upgrade(&tags.1 .0) {
                if *par_tag == tag {
                    return RemoveStatus::IsParent;
                }
            }
        }

        if let Some((_, removed)) = self.map.remove(&tag) {
            <Int<'t> as utils::drop_leakage::DropExt>::drop(removed);
            log::trace!(target: "qvnt_i::tag::remove", "Tag {} removed", tag);
    
            RemoveStatus::Removed
        } else {
            RemoveStatus::NotFound
        }
    }
}

impl<'t> Drop for IntTree<'t> {
    fn drop(&mut self) {
        for (_, (_, dropped)) in self.map.drain() {
            <Int<'t> as utils::drop_leakage::DropExt>::drop(dropped);
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    UnknownTagCmd(String),
    UnspecifiedTag,
}

impl From<Error> for crate::lines::Error {
    fn from(e: Error) -> Self {
        Self::Tag(e)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::UnknownTagCmd(cmd) => write!(f, "Unknown Tag subcommand {cmd:?}"),
            Error::UnspecifiedTag => write!(f, "Tag name should be specified"),
        }
    }
}

impl std::error::Error for Error {}

pub const HELP: &str = "QVNT Interpreter Tag command

USAGE:
    :tag [TAGCMD...]

TAGCMD:
    ls          Show the list of previously created tags
    mk TAG      Create TAG with current state
    ch TAG      Swap current state to TAG's state
    rm TAG      Remove TAG from tree
    root        Swap current state to default state
    help|h|?    Show this reference
";

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Command {
    List,
    Create(String),
    Remove(String),
    Checkout(String),
    Root,
    Help,
}

impl Command {
    pub fn parse_command<'a, I: Iterator<Item = &'a str>>(
        source: &mut I,
    ) -> Result<Command, Error> {
        match source.next() {
            None | Some("ls") => Ok(Command::List),
            Some("mk") => match source.next() {
                Some(arg) => Ok(Command::Create(arg.to_string())),
                None => Err(Error::UnspecifiedTag),
            },
            Some("rm") => match source.next() {
                Some(arg) => Ok(Command::Remove(arg.to_string())),
                None => Err(Error::UnspecifiedTag),
            },
            Some("ch") => match source.next() {
                Some(arg) => Ok(Command::Checkout(arg.to_string())),
                None => Err(Error::UnspecifiedTag),
            },
            Some("root") => Ok(Command::Root),
            Some("help" | "h" | "?") => Ok(Command::Help),
            Some(cmd) => Err(Error::UnknownTagCmd(cmd.to_string())),
        }
    }
}
