use rand::{
    seq::{IteratorRandom, SliceRandom},
    Rng,
};

use crate::abstract_fs::types::{AbstractExecutor, DirIndex, ModeFlag, Node, Workload};

enum Operation {
    MKDIR,
    CREATE,
    REMOVE,
}

pub fn generate_new(rng: &mut impl Rng, size: usize) -> Workload {
    let mut executor = AbstractExecutor::new();
    let mut name_idx = 1;
    let mode = vec![
        ModeFlag::S_IRWXU,
        ModeFlag::S_IRWXG,
        ModeFlag::S_IROTH,
        ModeFlag::S_IXOTH,
    ];
    for _ in 0..size {
        let alive = executor.alive();
        let alive_dirs: Vec<DirIndex> = alive
            .iter()
            .filter_map(|n| match n {
                Node::DIR(dir) => Some(dir.clone()),
                Node::FILE(_) => None,
            })
            .collect();
        let alive_dirs_except_root: Vec<DirIndex> = alive_dirs
            .iter()
            .filter(|&&d| d != AbstractExecutor::root_index())
            .map(|d| d.clone())
            .collect();
        let mut possible_ops = vec![Operation::MKDIR, Operation::CREATE];
        if !alive_dirs_except_root.is_empty() {
            possible_ops.push(Operation::REMOVE);
        }
        match possible_ops.choose(rng).unwrap() {
            Operation::MKDIR => {
                executor.mkdir(
                    alive_dirs.choose(rng).unwrap(),
                    name_idx.to_string(),
                    mode.clone(),
                );
                name_idx += 1;
            }
            Operation::CREATE => {
                executor.create(
                    alive_dirs.choose(rng).unwrap(),
                    name_idx.to_string(),
                    mode.clone(),
                );
                name_idx += 1;
            }
            Operation::REMOVE => {
                let node = alive
                    .iter()
                    .filter(|n| match n {
                        Node::FILE(_) => true,
                        Node::DIR(dir) => *dir != AbstractExecutor::root_index(),
                    })
                    .choose(rng)
                    .unwrap();
                executor.remove(node);
            }
        }
    }
    executor.recording
}

#[cfg(test)]
mod tests {
    use rand::{rngs::StdRng, SeedableRng};

    use super::*;

    #[test]
    fn test_generate_new() {
        for i in 0..1000 {
            let mut rng = StdRng::seed_from_u64(i);
            generate_new(&mut rng, 1000);
        }
    }
}
