use std::{cell::RefCell, collections::HashMap, fmt, rc::Rc};

use crate::utils;

#[derive(Debug)]
pub struct TreeEntry<T> {
    value: T,
    parent: Rc<String>,
}

#[derive(Debug)]
pub struct Tree<T: utils::drop_leakage::DropExt> {
    next_id: usize,
    head: RefCell<Rc<String>>,
    root: Rc<String>,
    map: HashMap<Rc<String>, (usize, TreeEntry<T>)>,
}

pub enum RemoveStatus {
    Removed,
    NotFound,
    IsParent,
    IsHead,
    IsRoot,
}

impl<T: utils::drop_leakage::DropExt> Tree<T> {
    pub fn with_root<S: ToString>(root: S) -> Self {
        let root = Rc::new(root.to_string());
        let map = HashMap::new();
        Self {
            next_id: 1,
            head: RefCell::new(Rc::clone(&root)),
            root,
            map,
        }
    }

    pub fn display(&self) -> termtree::Tree<String> {
        let mut tree = HashMap::new();
        for (leaf, (leaf_id, leaf_entry)) in &self.map {
            let TreeEntry { parent, .. } = leaf_entry;
            let children = tree
                .entry(parent.as_str())
                .or_insert_with(|| Vec::with_capacity(2));
            match children.binary_search_by_key(leaf_id, |(id, _)| *id) {
                Ok(pos) | Err(pos) => {
                    children.insert(pos, (*leaf_id, leaf.as_str()));
                }
            }
        }

        fn return_tree<'s>(
            tree: &HashMap<&'s str, Vec<(usize, &'s str)>>,
            tag: &'s str,
            head: &'s str,
        ) -> termtree::Tree<String> {
            let tree_node = if tag == head {
                format!("{} <-", tag)
            } else {
                tag.to_string()
            };
            if let Some(children) = tree.get(tag) {
                termtree::Tree::new(tree_node)
                    .with_leaves(children.iter().map(|(_, c)| return_tree(tree, c, head)))
            } else {
                termtree::Tree::new(tree_node)
            }
        }

        return_tree(&tree, self.root.as_str(), self.head.borrow().as_str()).with_multiline(true)
    }

    pub fn commit<S: AsRef<str>>(&mut self, tag: S, change: T) -> bool {
        let tag = tag.as_ref().to_string();

        if self.map.contains_key(&tag) {
            log::trace!(target: "qvnt_i::tag::commit", "Tag {} already exists", tag);
            return false;
        }

        log::trace!(target: "qvnt_i::tag::commit", "Tag {} created", tag);
        let tag = Rc::new(tag);
        let old_head = TreeEntry {
            value: change,
            parent: std::mem::replace(&mut *self.head.borrow_mut(), Rc::clone(&tag)),
        };
        self.map.insert(tag, (self.next_id, old_head));
        self.next_id += 1;

        true
    }

    pub fn checkout_root(&self) {
        *self.head.borrow_mut() = Rc::clone(&self.root);
        log::trace!(target: "qvnt_i::tag::checkout", "New head is on root");
    }

    pub fn checkout<S: AsRef<str>>(&self, tag: S) -> bool {
        let tag = tag.as_ref().to_string();

        match self
            .map
            .get_key_value(&tag)
            .map(|(entry, _)| entry)
            .or_else(|| (**self.root == tag).then_some(&self.root))
        {
            Some(entry) => {
                *self.head.borrow_mut() = Rc::clone(entry);
                log::trace!(target: "qvnt_i::tag::checkout", "New head is on tag {}", tag);
                true
            }
            _ => {
                log::trace!(target: "qvnt_i::tag::checkout", "Tag {} doesn't exist", tag);
                false
            }
        }
    }

    pub fn collect_to_head(
        &self,
        init: impl FnOnce() -> T,
        mut combine: impl FnMut(T, &T) -> T,
    ) -> Option<T> {
        let mut start = Rc::clone(&*self.head.borrow());
        let mut changes = init();

        log::trace!(target: "qvnt_i::tag::collect", "Staring collection");
        loop {
            log::trace!(target: "qvnt_i::tag::collect", "Collection step to tag {}", start);
            if start == self.root {
                break Some(changes);
            } else {
                let TreeEntry { value, parent } = &self.map.get(&start)?.1;

                changes = combine(changes, value);
                start = Rc::clone(parent);
            }
        }
    }

    pub fn remove<S: AsRef<str>>(&mut self, tag: S) -> RemoveStatus {
        let tag = tag.as_ref().to_string();

        if **self.head.borrow() == tag {
            return RemoveStatus::IsHead;
        }

        if **self.root == tag {
            return RemoveStatus::IsRoot;
        }

        for (_, (_, entry)) in self.map.iter() {
            if **entry.parent == tag {
                return RemoveStatus::IsParent;
            }
        }

        if let Some((_, entry)) = self.map.remove(&tag) {
            let TreeEntry { value, .. } = entry;
            T::drop(value);
            log::trace!(target: "qvnt_i::tag::remove", "Tag {} removed", tag);

            RemoveStatus::Removed
        } else {
            RemoveStatus::NotFound
        }
    }
}

impl<T: utils::drop_leakage::DropExt> Drop for Tree<T> {
    fn drop(&mut self) {
        for (_, (_, entry)) in self.map.drain() {
            let TreeEntry { value, .. } = entry;
            T::drop(value);
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
