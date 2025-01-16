use std::{collections::HashMap, fmt::Display};

use serde::{Deserialize, Serialize};

use super::pathname::Name;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FileIndex(pub usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DirIndex(pub usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct FileDescriptor(pub usize);

impl Display for FileDescriptor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

#[derive(Debug, Clone)]
pub struct File {
    pub descriptor: Option<FileDescriptor>,
    pub content: Content,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct SourceSlice {
    /// inclusive
    pub from: u64,
    /// exclusive
    pub to: u64,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Content {
    pub slices: Vec<SourceSlice>,
}

impl Content {
    pub fn new() -> Self {
        Self { slices: vec![] }
    }
    pub fn write(&mut self, src_offset: u64, size: u64) {
        self.slices.push(SourceSlice {
            from: src_offset,
            to: src_offset + size,
        });
    }
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
