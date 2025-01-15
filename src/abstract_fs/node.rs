use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::pathname::Name;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FileIndex(pub usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DirIndex(pub usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct FileDescriptor(pub usize);

#[derive(Debug, Clone)]
pub struct File {
    pub is_open: bool,
}

#[derive(Debug, Clone)]
pub struct Dir {
    pub children: HashMap<Name, Node>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Node {
    FILE(FileIndex),
    DIR(DirIndex),
}
