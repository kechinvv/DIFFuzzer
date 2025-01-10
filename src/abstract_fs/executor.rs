use std::collections::{HashMap, VecDeque};

use super::types::*;

type Result<T> = std::result::Result<T, ExecutorError>;

#[derive(Debug)]
pub enum ExecutorError {
    NotADir,
    NameAlreadyExists,
    RemoveRoot,
    NotFound,
    NotAFile,
}

fn split_path(path: &str) -> (&str, &str) {
    let split_at = path.rfind('/').unwrap();
    (&path[..split_at], &path[split_at + 1..])
}

impl AbstractExecutor {
    pub fn new() -> Self {
        AbstractExecutor {
            dirs: vec![Dir {
                parent: None,
                children: HashMap::new(),
            }],
            files: vec![],
            nodes_created: 0,
            recording: Workload::new(),
        }
    }

    pub fn remove(&mut self, path: PathName) -> Result<()> {
        let (parent_path, name) = split_path(&path);
        let node = &self.resolve_node(path.clone())?;
        let parent_idx = match self.resolve_node(parent_path.to_owned())? {
            Node::DIR(dir_index) => dir_index,
            _ => return Err(ExecutorError::NotADir),
        };
        self.recording
            .push(Operation::REMOVE { path: path.clone() });
        let parent = self.dir_mut(&parent_idx);
        parent.children.remove(name);
        match node {
            Node::DIR(to_remove) => {
                if *to_remove == AbstractExecutor::root_index() {
                    return Err(ExecutorError::RemoveRoot);
                }
                let to_remove = self.dir_mut(to_remove);
                to_remove.parent = None;
            }
            Node::FILE(to_remove) => {
                let to_remove = self.file_mut(to_remove);
                let index = to_remove
                    .parents
                    .iter()
                    .position(|it| *it == parent_idx)
                    .unwrap();
                to_remove.parents.remove(index);
            }
        }
        Ok(())
    }

    pub fn mkdir(&mut self, parent: &DirIndex, name: Name, mode: Mode) -> Result<DirIndex> {
        if self.name_exists(&parent, &name) {
            return Err(ExecutorError::NameAlreadyExists);
        }
        let dir = Dir {
            parent: Some(parent.clone()),
            children: HashMap::new(),
        };
        let dir_idx = DirIndex(self.dirs.len());
        self.dirs.push(dir);
        self.dir_mut(&parent)
            .children
            .insert(name, Node::DIR(dir_idx));
        self.recording.push(Operation::MKDIR {
            path: self.resolve_path(&Node::DIR(dir_idx)),
            mode,
        });
        self.nodes_created += 1;
        Ok(dir_idx)
    }

    pub fn create(&mut self, parent: &DirIndex, name: Name, mode: Mode) -> Result<FileIndex> {
        if self.name_exists(&parent, &name) {
            return Err(ExecutorError::NameAlreadyExists);
        }
        let file = File {
            parents: vec![parent.clone()],
        };
        let file_idx = FileIndex(self.files.len());
        self.files.push(file);
        self.dir_mut(&parent)
            .children
            .insert(name, Node::FILE(file_idx));
        self.recording.push(Operation::CREATE {
            path: self.resolve_path(&Node::FILE(file_idx)),
            mode,
        });
        self.nodes_created += 1;
        Ok(file_idx)
    }

    pub fn hardlink(
        &mut self,
        old_file: &FileIndex,
        parent: &DirIndex,
        name: Name,
    ) -> Result<FileIndex> {
        if self.name_exists(&parent, &name) {
            return Err(ExecutorError::NameAlreadyExists);
        }
        let node = &Node::FILE(old_file.to_owned());
        let old_path = self.resolve_path(node);
        let file = self.file_mut(old_file);
        file.parents.push(parent.to_owned());
        let parent_dir = self.dir_mut(parent);
        parent_dir
            .children
            .insert(name.clone(), Node::FILE(old_file.to_owned()));
        let parent_path = self.resolve_path(&Node::DIR(parent.to_owned()));
        let new_path = if *parent == AbstractExecutor::root_index() {
            format!("/{}", name)
        } else {
            format!("{}/{}", parent_path, name)
        };
        self.recording
            .push(Operation::HARDLINK { old_path, new_path });
        self.nodes_created += 1;
        Ok(old_file.to_owned())
    }

    pub fn replay(&mut self, workload: &Workload) -> Result<()> {
        for op in &workload.ops {
            match op {
                Operation::MKDIR { path, mode } => {
                    self.replay_mkdir(path.clone(), mode.clone())?;
                }
                Operation::CREATE { path, mode } => {
                    self.replay_create(path.clone(), mode.clone())?;
                }
                Operation::REMOVE { path } => self.replay_remove(path.clone())?,
                Operation::HARDLINK { old_path, new_path } => {
                    self.replay_hardlink(old_path.clone(), new_path.clone())?;
                }
            };
        }
        Ok(())
    }

    pub fn replay_remove(&mut self, path: PathName) -> Result<()> {
        self.remove(path)
    }

    pub fn replay_mkdir(&mut self, path: PathName, mode: Mode) -> Result<DirIndex> {
        let (parent_path, name) = split_path(&path);
        let parent = match self.resolve_node(parent_path.to_owned())? {
            Node::DIR(dir_index) => dir_index,
            _ => return Err(ExecutorError::NotADir),
        };
        self.mkdir(&parent, name.to_owned(), mode)
    }

    pub fn replay_create(&mut self, path: PathName, mode: Mode) -> Result<FileIndex> {
        let (parent_path, name) = split_path(&path);
        let parent = match self.resolve_node(parent_path.to_owned())? {
            Node::DIR(dir_index) => dir_index,
            _ => return Err(ExecutorError::NotADir),
        };
        self.create(&parent, name.to_owned(), mode)
    }

    pub fn replay_hardlink(&mut self, old_path: PathName, new_path: PathName) -> Result<FileIndex> {
        let old_file = match self.resolve_node(old_path)? {
            Node::FILE(file_index) => file_index,
            _ => return Err(ExecutorError::NotAFile),
        };
        let (parent_path, name) = split_path(&new_path);
        let parent = match self.resolve_node(parent_path.to_owned())? {
            Node::DIR(dir_index) => dir_index,
            _ => return Err(ExecutorError::NotADir),
        };
        self.hardlink(&old_file, &parent, name.to_owned())
    }

    fn name_exists(&self, idx: &DirIndex, name: &Name) -> bool {
        self.dir(idx).children.contains_key(name)
    }

    fn dir(&self, idx: &DirIndex) -> &Dir {
        self.dirs.get(idx.0).unwrap()
    }

    fn dir_mut(&mut self, idx: &DirIndex) -> &mut Dir {
        self.dirs.get_mut(idx.0).unwrap()
    }

    fn file(&self, idx: &FileIndex) -> &File {
        self.files.get(idx.0).unwrap()
    }

    fn file_mut(&mut self, idx: &FileIndex) -> &mut File {
        self.files.get_mut(idx.0).unwrap()
    }

    fn root_mut(&mut self) -> &mut Dir {
        self.dirs.get_mut(0).unwrap()
    }

    fn root(&self) -> &Dir {
        self.dirs.get(0).unwrap()
    }

    pub fn resolve_node(&self, path: PathName) -> Result<Node> {
        let mut last = Node::DIR(AbstractExecutor::root_index());
        let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
        for segment in &segments {
            let dir = match last {
                Node::DIR(dir_index) => self.dir(&dir_index),
                _ => return Err(ExecutorError::NotADir),
            };
            last = dir
                .children
                .get(segment.to_owned())
                .ok_or(ExecutorError::NotFound)?
                .clone();
        }
        Ok(last)
    }

    pub fn root_index() -> DirIndex {
        DirIndex(0)
    }

    pub fn resolve_path(&self, node: &Node) -> PathName {
        let mut segments: Vec<String> = vec![];
        let mut next = node.clone();
        loop {
            match next {
                Node::DIR(idx) => {
                    let dir = self.dir(&idx);
                    match dir.parent {
                        Some(parent) => {
                            let parent_dir = self.dir(&parent);
                            let (name, _) = parent_dir
                                .children
                                .iter()
                                .find(|(_, node)| next == **node)
                                .unwrap();
                            segments.push(name.clone());
                            next = Node::DIR(parent.clone());
                        }
                        None => break,
                    }
                }
                Node::FILE(idx) => {
                    let file = self.file(&idx);
                    let parent_idx = file.parents.last().unwrap();
                    let parent = self.dir(parent_idx);
                    let (name, _) = parent
                        .children
                        .iter()
                        .find(|(_, node)| next == **node)
                        .unwrap();
                    segments.push(name.clone());
                    next = Node::DIR(parent_idx.to_owned()).clone();
                }
            }
        }
        segments.reverse();
        "/".to_owned() + segments.join("/").as_str()
    }

    pub fn alive(&self) -> Vec<Node> {
        let root = AbstractExecutor::root_index();
        let mut visited = vec![];
        let mut queue = VecDeque::new();
        queue.push_back(&root);
        visited.push(Node::DIR(root));
        while !queue.is_empty() {
            let next = queue.pop_front().unwrap();
            let dir = self.dir(&next);
            for (_, node) in dir.children.iter() {
                match node {
                    Node::DIR(idx) => {
                        queue.push_back(idx);
                        visited.push(Node::DIR(idx.clone()));
                    }
                    Node::FILE(idx) => {
                        visited.push(Node::FILE(idx.clone()));
                    }
                }
            }
        }
        visited
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init_root() {
        let exec = AbstractExecutor::new();
        assert_eq!(
            vec![Node::DIR(AbstractExecutor::root_index())],
            exec.alive()
        )
    }

    #[test]
    #[should_panic]
    fn test_remove_root() {
        let mut exec = AbstractExecutor::new();
        exec.remove("/".to_owned()).unwrap();
    }

    #[test]
    fn test_mkdir() {
        let mut exec = AbstractExecutor::new();
        let foo = exec
            .mkdir(&AbstractExecutor::root_index(), "foobar".to_owned(), vec![])
            .unwrap();
        match exec.root().children.get("foobar").unwrap() {
            Node::DIR(idx) => {
                assert_eq!(foo, *idx)
            }
            _ => {
                assert!(false, "not a dir")
            }
        }
        assert_eq!(
            Workload {
                ops: vec![Operation::MKDIR {
                    path: "/foobar".to_owned(),
                    mode: vec![],
                }],
            },
            exec.recording
        );
        assert_eq!(
            vec![Node::DIR(AbstractExecutor::root_index()), Node::DIR(foo)],
            exec.alive()
        );
        assert_eq!(1, exec.nodes_created);
        test_replay(exec.recording);
    }

    #[test]
    #[should_panic]
    fn test_mkdir_name_exists() {
        let mut exec = AbstractExecutor::new();
        exec.mkdir(&AbstractExecutor::root_index(), "foobar".to_owned(), vec![])
            .unwrap();
        exec.mkdir(&AbstractExecutor::root_index(), "foobar".to_owned(), vec![])
            .unwrap();
    }

    #[test]
    fn test_create() {
        let mut exec = AbstractExecutor::new();
        let foo = exec
            .create(&AbstractExecutor::root_index(), "foobar".to_owned(), vec![])
            .unwrap();
        match exec.root().children.get("foobar").unwrap() {
            Node::FILE(idx) => {
                assert_eq!(foo, *idx)
            }
            _ => {
                assert!(false, "not a file")
            }
        }
        assert_eq!(
            vec![Node::DIR(AbstractExecutor::root_index()), Node::FILE(foo)],
            exec.alive()
        );
        assert_eq!(
            Workload {
                ops: vec![Operation::CREATE {
                    path: "/foobar".to_owned(),
                    mode: vec![],
                }]
            },
            exec.recording
        );
        assert_eq!(1, exec.nodes_created);
        test_replay(exec.recording);
    }

    #[test]
    #[should_panic]
    fn test_create_name_exists() {
        let mut exec = AbstractExecutor::new();
        exec.create(&AbstractExecutor::root_index(), "foobar".to_owned(), vec![])
            .unwrap();
        exec.create(&AbstractExecutor::root_index(), "foobar".to_owned(), vec![])
            .unwrap();
    }

    #[test]
    fn test_remove_file() {
        let mut exec = AbstractExecutor::new();
        let foo = exec
            .create(&AbstractExecutor::root_index(), "foobar".to_owned(), vec![])
            .unwrap();
        let boo = exec
            .create(&AbstractExecutor::root_index(), "boo".to_owned(), vec![])
            .unwrap();
        let mut expected = vec![
            Node::DIR(AbstractExecutor::root_index()),
            Node::FILE(foo),
            Node::FILE(boo),
        ];
        let mut actual = exec.alive();
        expected.sort();
        actual.sort();
        assert_eq!(expected, actual);

        exec.remove("/foobar".to_owned()).unwrap();

        assert_eq!(1, exec.root().children.len());
        match exec.root().children.get("boo").unwrap() {
            Node::FILE(idx) => {
                assert_eq!(boo, *idx);
            }
            _ => {
                assert!(false, "not a file")
            }
        }
        let mut expected = vec![Node::DIR(AbstractExecutor::root_index()), Node::FILE(boo)];
        let mut actual = exec.alive();
        expected.sort();
        actual.sort();
        assert_eq!(expected, actual);
        assert_eq!(
            Workload {
                ops: vec![
                    Operation::CREATE {
                        path: "/foobar".to_owned(),
                        mode: vec![],
                    },
                    Operation::CREATE {
                        path: "/boo".to_owned(),
                        mode: vec![],
                    },
                    Operation::REMOVE {
                        path: "/foobar".to_owned(),
                    }
                ],
            },
            exec.recording
        );
        assert_eq!(2, exec.nodes_created);
        test_replay(exec.recording);
    }

    #[test]
    fn test_hardlink() {
        let mut exec = AbstractExecutor::new();
        let foo = exec
            .create(&AbstractExecutor::root_index(), "foo".to_owned(), vec![])
            .unwrap();
        let bar = exec
            .mkdir(&AbstractExecutor::root_index(), "bar".to_owned(), vec![])
            .unwrap();
        let boo = exec.hardlink(&foo, &bar, "boo".to_owned()).unwrap();

        assert_eq!(foo, boo);
        let mut expected = vec![
            Node::DIR(AbstractExecutor::root_index()),
            Node::DIR(bar),
            Node::FILE(foo),
            Node::FILE(boo),
        ];
        let mut actual = exec.alive();
        expected.sort();
        actual.sort();
        assert_eq!(expected, actual);

        let root = exec.root();
        let bar_dir = exec.dir(&bar);
        assert_eq!(2, root.children.len());
        assert_eq!(1, bar_dir.children.len());
        assert_eq!(
            root.children.get("foo").unwrap(),
            bar_dir.children.get("boo").unwrap()
        );

        let parents = vec![AbstractExecutor::root_index(), bar];
        assert_eq!(parents, exec.file(&foo).parents);
        assert_eq!(parents, exec.file(&boo).parents);

        assert_eq!(
            Workload {
                ops: vec![
                    Operation::CREATE {
                        path: "/foo".to_owned(),
                        mode: vec![],
                    },
                    Operation::MKDIR {
                        path: "/bar".to_owned(),
                        mode: vec![],
                    },
                    Operation::HARDLINK {
                        old_path: "/foo".to_owned(),
                        new_path: "/bar/boo".to_owned(),
                    }
                ],
            },
            exec.recording
        );
        assert_eq!(3, exec.nodes_created);
        test_replay(exec.recording);
    }

    #[test]
    fn test_remove_hardlink() {
        let mut exec = AbstractExecutor::new();
        let foo = exec
            .create(&AbstractExecutor::root_index(), "foo".to_owned(), vec![])
            .unwrap();
        exec.hardlink(&foo, &AbstractExecutor::root_index(), "bar".to_owned())
            .unwrap();
        exec.remove("/bar".to_owned()).unwrap();

        let mut expected = vec![Node::DIR(AbstractExecutor::root_index()), Node::FILE(foo)];
        let mut actual = exec.alive();
        expected.sort();
        actual.sort();
        assert_eq!(expected, actual);

        assert_eq!(1, exec.root().children.len());

        assert_eq!(
            vec![AbstractExecutor::root_index()],
            exec.file(&foo).parents
        );

        assert_eq!(
            Workload {
                ops: vec![
                    Operation::CREATE {
                        path: "/foo".to_owned(),
                        mode: vec![],
                    },
                    Operation::HARDLINK {
                        old_path: "/foo".to_owned(),
                        new_path: "/bar".to_owned(),
                    },
                    Operation::REMOVE {
                        path: "/bar".to_owned(),
                    }
                ],
            },
            exec.recording
        );
        assert_eq!(2, exec.nodes_created);
        test_replay(exec.recording);
    }

    #[test]
    #[should_panic]
    fn test_hardlink_name_exists() {
        let mut exec = AbstractExecutor::new();
        exec.create(&AbstractExecutor::root_index(), "foo".to_owned(), vec![])
            .unwrap();
        let bar = exec
            .create(&AbstractExecutor::root_index(), "bar".to_owned(), vec![])
            .unwrap();
        exec.hardlink(&bar, &AbstractExecutor::root_index(), "foo".to_owned())
            .unwrap();
    }

    #[test]
    fn test_remove_dir() {
        let mut exec = AbstractExecutor::new();
        let foo = exec
            .mkdir(&AbstractExecutor::root_index(), "foobar".to_owned(), vec![])
            .unwrap();
        let boo = exec
            .mkdir(&AbstractExecutor::root_index(), "boo".to_owned(), vec![])
            .unwrap();
        let mut expected = vec![
            Node::DIR(AbstractExecutor::root_index()),
            Node::DIR(foo),
            Node::DIR(boo),
        ];
        let mut actual = exec.alive();
        expected.sort();
        actual.sort();
        assert_eq!(expected, actual);

        exec.remove("/foobar".to_owned()).unwrap();

        assert_eq!(1, exec.root().children.len());
        match exec.root().children.get("boo").unwrap() {
            Node::DIR(idx) => {
                assert_eq!(boo, *idx);
            }
            _ => {
                assert!(false, "not a dir")
            }
        }
        let mut expected = vec![Node::DIR(AbstractExecutor::root_index()), Node::DIR(boo)];
        let mut actual = exec.alive();
        expected.sort();
        actual.sort();
        assert_eq!(expected, actual);
        assert_eq!(
            Workload {
                ops: vec![
                    Operation::MKDIR {
                        path: "/foobar".to_owned(),
                        mode: vec![],
                    },
                    Operation::MKDIR {
                        path: "/boo".to_owned(),
                        mode: vec![],
                    },
                    Operation::REMOVE {
                        path: "/foobar".to_owned(),
                    }
                ],
            },
            exec.recording
        );
        assert_eq!(2, exec.nodes_created);
        test_replay(exec.recording);
    }

    #[test]
    fn test_resolve_path() {
        let mut exec = AbstractExecutor::new();
        let foo = exec
            .mkdir(&AbstractExecutor::root_index(), "foo".to_owned(), vec![])
            .unwrap();
        let bar = exec.mkdir(&foo, "bar".to_owned(), vec![]).unwrap();
        let boo = exec.create(&bar, "boo".to_owned(), vec![]).unwrap();
        assert_eq!("/foo", exec.resolve_path(&Node::DIR(foo)));
        assert_eq!("/foo/bar", exec.resolve_path(&Node::DIR(bar)));
        assert_eq!("/foo/bar/boo", exec.resolve_path(&Node::FILE(boo)));
        assert_eq!(3, exec.nodes_created);
        test_replay(exec.recording);
    }

    #[test]
    fn test_resolve_node() {
        let mut exec = AbstractExecutor::new();
        assert_eq!(
            Node::DIR(AbstractExecutor::root_index()),
            exec.resolve_node("/".to_owned()).unwrap()
        );
        let foo = exec
            .mkdir(&AbstractExecutor::root_index(), "foo".to_owned(), vec![])
            .unwrap();
        let bar = exec.mkdir(&foo, "bar".to_owned(), vec![]).unwrap();
        let boo = exec.create(&bar, "boo".to_owned(), vec![]).unwrap();
        assert_eq!(
            Node::DIR(foo),
            exec.resolve_node("/foo".to_owned()).unwrap()
        );
        assert_eq!(
            Node::DIR(foo),
            exec.resolve_node("/foo/".to_owned()).unwrap()
        );
        assert_eq!(
            Node::DIR(bar),
            exec.resolve_node("/foo/bar".to_owned()).unwrap()
        );
        assert_eq!(
            Node::DIR(bar),
            exec.resolve_node("/foo/bar/".to_owned()).unwrap()
        );
        assert_eq!(
            Node::FILE(boo),
            exec.resolve_node("/foo/bar/boo".to_owned()).unwrap()
        );
        assert_eq!(
            Node::FILE(boo),
            exec.resolve_node("/foo/bar/boo/".to_owned()).unwrap()
        );
        assert_eq!(3, exec.nodes_created);
        test_replay(exec.recording);
    }

    fn test_replay(workload: Workload) {
        let mut exec = AbstractExecutor::new();
        exec.replay(&workload).unwrap();
        assert_eq!(workload, exec.recording);
    }
}
