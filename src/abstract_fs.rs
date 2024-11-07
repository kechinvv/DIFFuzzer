//! Based on POSIX.1-2024

#![allow(dead_code)]

use std::{collections::HashSet, ops::Index, vec};

/// Flags for `open(path, flags, mode)` syscall.
///
/// Applications *shall* specify __exactly one__ of the __first 5__ values.
#[derive(PartialEq, Eq, Hash)]
#[allow(nonstandard_style)]
enum OpenFlag {
    /// Open for execute only (non-directory files).
    /// If path names a directory and `O_EXEC` is not the same value as `O_SEARCH`, `open()` shall fail.
    O_EXEC,
    /// Open for reading only.
    O_RDONLY,
    /// Open for reading and writing.
    /// If path names a `FIFO`, and the implementation does not support opening a `FIFO` for simultaneous read and write, then `open()` shall fail.
    O_RDWR,
    /// Open directory for search only. If path names a non-directory file and `O_SEARCH` is not the same value as `O_EXEC`, `open()` shall fail.
    O_SEARCH,
    /// Open for writing only.
    O_WRONLY,

    /// If set, the file offset shall be set to the end of the file prior to each write.
    O_APPEND,
    /// If set, the `FD_CLOEXEC` flag for the new file descriptor shall be set.
    O_CLOEXEC,
    /// If set, the `FD_CLOFORK` flag for the new file descriptor shall be set.
    O_CLOFORK,
    /// If the file exists, this flag has no effect except as noted under `O_EXCL` below.
    /// Otherwise, if `O_DIRECTORY` is not set the file shall be created as a regular file.
    ///
    /// __LINUX__: The `mode` argument __must__ be supplied if `O_CREAT` or `O_TMPFILE` is specified in flags.
    O_CREAT,
    /// If path resolves to a non-directory file, fail and set errno to `ENOTDIR`.
    O_DIRECTORY,
    /// Write I/O operations on the file descriptor shall complete as defined by synchronized I/O data integrity completion.
    O_DSYNC,
    /// If `O_CREAT` and `O_EXCL` are set, `open()` shall fail if the file exists.
    /// If `O_EXCL` and `O_CREAT` are set, and path names a symbolic link, `open()` shall fail and set errno to `EEXIST`, regardless of the contents of the symbolic link.
    /// If `O_EXCL` is set and `O_CREAT` is not set, the result is undefined.
    O_EXCL,
    /// If set and path identifies a terminal device, `open()` shall not cause the terminal device to become the controlling terminal for the process.
    /// If path does not identify a terminal device, O_NOCTTY shall be ignored.
    O_NOCTTY,
    /// If path names a symbolic link, fail and set errno to `ELOOP`.
    O_NOFOLLOW,
    /// When opening a FIFO with `O_RDONLY` or `O_WRONLY` set:
    ///
    /// - If `O_NONBLOCK` is set, an `open()` for reading-only shall return without delay. An `open()` for writing-only shall return an error if no process currently has the file open for reading.
    /// - If `O_NONBLOCK` is clear, an `open()` for reading-only shall block the calling thread until a thread opens the file for writing. An `open()` for writing-only shall block the calling thread until a thread opens the file for reading.
    ///
    /// When opening a block special or character special file that supports non-blocking opens:
    ///
    /// - If `O_NONBLOCK` is set, the `open()` function shall return without blocking for the device to be ready or available. Subsequent behavior of the device is device-specific.
    /// - If `O_NONBLOCK` is clear, the `open()` function shall block the calling thread until the device is ready or available before returning.
    ///
    O_NONBLOCK,
    /// Read I/O operations on the file descriptor shall complete at the same level of integrity as specified by the `O_DSYNC` and `O_SYNC` flags.
    /// If both `O_DSYNC` and `O_RSYNC` are set in oflag, all I/O operations on the file descriptor shall complete as defined by synchronized I/O data integrity completion.
    /// If both `O_SYNC` and `O_RSYNC` are set in flags, all I/O operations on the file descriptor shall complete as defined by synchronized I/O file integrity completion.
    O_RSYNC,
    ///  Write I/O operations on the file descriptor shall complete as defined by synchronized I/O file integrity completion.
    O_SYNC,
    /// If the file exists and is a regular file, and the file is successfully opened `O_RDWR` or `O_WRONLY`, its length shall be truncated to 0, and the mode and owner shall be unchanged.
    /// It shall have no effect on `FIFO` special files or terminal device files.
    /// Its effect on other file types is implementation-defined.
    /// The result of using `O_TRUNC` without either `O_RDWR` or `O_WRONLY` is undefined.
    O_TRUNC,

    O_TTY_INIT,
}
#[derive(PartialEq, Eq, Hash)]
#[allow(nonstandard_style)]
enum Mode {
    /// Read, write, execute/search by owner.
    S_IRWXU = 0o700,
    /// Read permission, owner.
    S_IRUSR = 0o400,
    /// Write permission, owner.
    S_IWUSR = 0o200,
    /// Execute/search permission, owner.
    S_IXUSR = 0o100,
    /// Read, write, execute/search by group.
    S_IRWXG = 0o70,
    /// Read permission, group.
    S_IRGRP = 0o40,
    /// Write permission, group.
    S_IWGRP = 0o20,
    /// Execute/search permission, group.
    S_IXGRP = 0o10,
    /// Read, write, execute/search by others.
    S_IRWXO = 0o7,
    /// Read permission, others.
    S_IROTH = 0o4,
    /// Write permission, others.
    S_IWOTH = 0o2,
    /// Execute/search permission, others.
    S_IXOTH = 0o1,

    /// Set-user-ID on execution.
    S_ISUID = 0o4000,
    /// Set-group-ID on execution.
    S_ISGID = 0o2000,
    /// On directories, restricted deletion flag.
    S_ISVTX = 0o1000,
}

type Name = String;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FileIndex(usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DirIndex(usize);

#[derive(Debug)]
struct FileDescriptor(usize);

#[derive(Debug, Clone)]
struct File {
    name: Name,
    parent: DirIndex,
}

#[derive(Debug, Clone)]
struct Dir {
    name: Name,
    parent: Option<DirIndex>,
    children: Vec<Node>,
}

#[derive(Debug, Clone)]
enum Node {
    FILE(FileIndex),
    DIR(DirIndex),
}

struct AbstractExecutor {
    dirs: Vec<Dir>,
    files: Vec<File>,
}

impl AbstractExecutor {
    fn new() -> Self {
        AbstractExecutor {
            dirs: vec![Dir {
                name: String::new(),
                parent: None,
                children: vec![],
            }],
            files: vec![],
        }
    }

    fn remove(&mut self, node: &Node) {
        match node {
            Node::DIR(DirIndex(idx)) => {
                if *idx == 0 {
                    panic!("removing root is prohibited")
                }
            }
            Node::FILE(to_remove) => {
                let file: File = self.get_file(&to_remove).clone();
                let parent = self.get_dir_mut(&file.parent);
                parent.children.retain(|n| match n {
                    Node::FILE(idx) => idx != to_remove,
                    Node::DIR(_) => true,
                });
            }
        }
    }

    fn mkdir(&mut self, parent: &DirIndex, name: Name, _mode: HashSet<Mode>) -> DirIndex {
        if self.name_exists(&parent, &name) {
            panic!("parent directory already has a file with this name")
        }
        let dir = Dir {
            name: name,
            parent: Some(parent.clone()),
            children: vec![],
        };
        let dir_idx = DirIndex(self.dirs.len());
        self.dirs.push(dir);
        self.get_dir_mut(&parent).children.push(Node::DIR(dir_idx));
        dir_idx
    }

    fn create(&mut self, parent: &DirIndex, name: Name) -> FileIndex {
        if self.name_exists(&parent, &name) {
            panic!("parent directory already has a file with this name")
        }
        let file = File {
            name: name,
            parent: parent.clone(),
        };
        let file_idx = FileIndex(self.files.len());
        self.files.push(file);
        self.get_dir_mut(&parent)
            .children
            .push(Node::FILE(file_idx));
        file_idx
    }

    fn name_exists(&self, idx: &DirIndex, name: &Name) -> bool {
        self.get_dir(idx).children.iter().any(|node| match node {
            Node::DIR(idx) => &self.get_dir(idx).name == name,
            Node::FILE(idx) => &self.get_file(idx).name == name,
        })
    }

    fn get_dir(&self, idx: &DirIndex) -> &Dir {
        self.dirs.get(idx.0).unwrap()
    }

    fn get_dir_mut(&mut self, idx: &DirIndex) -> &mut Dir {
        self.dirs.get_mut(idx.0).unwrap()
    }

    fn get_file(&self, idx: &FileIndex) -> &File {
        self.files.get(idx.0).unwrap()
    }

    fn get_file_mut(&mut self, idx: &FileIndex) -> &mut File {
        self.files.get_mut(idx.0).unwrap()
    }

    fn root_mut(&mut self) -> &mut Dir {
        self.dirs.get_mut(0).unwrap()
    }

    fn root(&self) -> &Dir {
        self.dirs.get(0).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init_root() {
        let exec = AbstractExecutor::new();
        assert_eq!("", exec.root().name);
    }

    #[test]
    #[should_panic]
    fn test_remove_root() {
        let mut exec = AbstractExecutor::new();
        exec.remove(&Node::DIR(DirIndex(0)));
    }

    #[test]
    fn test_mkdir() {
        let mut exec = AbstractExecutor::new();
        exec.mkdir(&DirIndex(0), String::from("foobar"), HashSet::new());
        match exec.root().children[0] {
            Node::DIR(idx) => {
                assert_eq!("foobar", exec.get_dir(&idx).name)
            }
            _ => {
                assert!(false, "not a dir")
            }
        }
    }

    #[test]
    #[should_panic]
    fn test_mkdir_same_name() {
        let mut exec = AbstractExecutor::new();
        exec.mkdir(&DirIndex(0), String::from("foobar"), HashSet::new());
        exec.mkdir(&DirIndex(0), String::from("foobar"), HashSet::new());
    }

    #[test]
    fn test_create() {
        let mut exec = AbstractExecutor::new();
        exec.create(&DirIndex(0), String::from("foobar"));
        match exec.root().children[0] {
            Node::FILE(idx) => {
                assert_eq!("foobar", exec.get_file(&idx).name)
            }
            _ => {
                assert!(false, "not a file")
            }
        }
    }

    #[test]
    #[should_panic]
    fn test_create_same_name() {
        let mut exec = AbstractExecutor::new();
        exec.create(&DirIndex(0), String::from("foobar"));
        exec.create(&DirIndex(0), String::from("foobar"));
    }

    #[test]
    fn test_remove_file() {
        let mut exec = AbstractExecutor::new();
        let foo = exec.create(&DirIndex(0), String::from("foobar"));
        exec.create(&DirIndex(0), String::from("boo"));
        exec.remove(&Node::FILE(foo));
        assert_eq!(1, exec.root().children.len());
        match exec.root().children[0] {
            Node::FILE(idx) => {
                assert_eq!("boo", exec.get_file(&idx).name)
            }
            _ => {
                assert!(false, "not a file")
            }
        }
    }
}
