use std::{cell::RefCell, collections::HashMap, fmt, mem::MaybeUninit, rc::Rc};

use crate::utils;

#[derive(Debug)]
pub enum TreeEntry<T> {
    Root,
    Leaf { value: T, parent: Rc<String> },
}

#[derive(Debug)]
pub struct Tree<T: utils::drop_leakage::DropExt> {
    head: RefCell<Rc<String>>,
    map: HashMap<Rc<String>, TreeEntry<T>>,
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
        let map = HashMap::from([(Rc::clone(&root), TreeEntry::Root)]);
        Self {
            head: RefCell::new(Rc::clone(&root)),
            map,
        }
    }

    #[allow(dead_code)]
    pub fn display_short(&self) -> termtree::Tree<Rc<String>> {
        let head = &*self.head.borrow();
        match self.map.get(head) {
            Some(head_entry) => match head_entry {
                TreeEntry::Root => termtree::Tree::new(Rc::clone(head)),
                TreeEntry::Leaf {
                    parent: head_parent,
                    ..
                } => {
                    let mut siblings = termtree::Tree::new(Rc::clone(head_parent));
                    siblings.extend(self.map.iter().filter_map(|(tag, entry)| match entry {
                        TreeEntry::Leaf { parent, .. } if parent == head_parent => {
                            Some(Rc::clone(tag))
                        }
                        _ => None,
                    }));
                    siblings
                }
            },
            None => unsafe {
                std::hint::unreachable_unchecked();
            },
        }
        .with_multiline(true)
    }

    pub fn display(&self) -> termtree::Tree<String> {
        let mut tree = HashMap::new();
        let mut root = MaybeUninit::uninit();
        for (leaf, leaf_entry) in &self.map {
            if let TreeEntry::Leaf { parent, .. } = leaf_entry {
                let children = tree
                    .entry(parent.as_str())
                    .or_insert_with(|| Vec::with_capacity(2));
                match children.binary_search(&leaf.as_str()) {
                    Ok(pos) | Err(pos) => {
                        children.insert(pos, leaf.as_str());
                    }
                }
            } else {
                root.write(Rc::clone(leaf));
            }
        }

        fn return_tree<'s >(
            tree: &HashMap<&'s str, Vec<&'s str>>,
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
                    .with_leaves(children.iter().map(|c| return_tree(tree, c, head)))
            } else {
                termtree::Tree::new(tree_node)
            }
        }

        return_tree(&tree, &unsafe { root.assume_init() }, self.head.borrow().as_str()).with_multiline(true)
    }

    pub fn commit<S: AsRef<str>>(&mut self, tag: S, change: T) -> bool {
        let tag = tag.as_ref().to_string();

        if self.map.contains_key(&tag) {
            log::trace!(target: "qvnt_i::tag::commit", "Tag {} already exists", tag);
            return false;
        }

        let tag = Rc::new(tag);
        let old_head = {
            TreeEntry::Leaf {
                value: change,
                parent: Rc::clone(&*self.head.borrow()),
            }
        };
        *self.head.borrow_mut() = Rc::clone(&tag);
        log::trace!(target: "qvnt_i::tag::commit", "Tag {} created", tag);
        self.map.insert(tag, old_head);

        true
    }

    pub fn checkout<S: AsRef<str>>(&self, tag: S) -> bool {
        let tag = tag.as_ref().to_string();

        match self.map.get_key_value(&tag) {
            Some(entry) => {
                *self.head.borrow_mut() = Rc::clone(entry.0);
                log::trace!(target: "qvnt_i::tag::checkout", "New head is on tag {}", tag);
                true
            }
            None => {
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
            match self.map.get(&start)? {
                TreeEntry::Root => break Some(changes),
                TreeEntry::Leaf { value, parent } => {
                    changes = combine(changes, value);
                    start = Rc::clone(parent);
                }
            }
        }
    }

    pub fn remove<S: AsRef<str>>(&mut self, tag: S) -> RemoveStatus {
        let tag = tag.as_ref().to_string();

        if **self.head.borrow() == tag {
            return RemoveStatus::IsHead;
        }

        for (_, entry) in self.map.iter() {
            match entry {
                TreeEntry::Leaf { parent, .. } if **parent == tag => return RemoveStatus::IsParent,
                _ => {}
            }
        }

        if let Some(entry) = self.map.remove(&tag) {
            match entry {
                TreeEntry::Root => RemoveStatus::IsRoot,
                TreeEntry::Leaf { value, .. } => {
                    T::drop(value);
                    log::trace!(target: "qvnt_i::tag::remove", "Tag {} removed", tag);

                    RemoveStatus::Removed
                }
            }
        } else {
            RemoveStatus::NotFound
        }
    }
}

impl<T: utils::drop_leakage::DropExt> Drop for Tree<T> {
    fn drop(&mut self) {
        for (_, entry) in self.map.drain() {
            if let TreeEntry::Leaf { value, .. } = entry {
                T::drop(value);
            }
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
