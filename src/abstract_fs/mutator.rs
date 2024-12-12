use std::collections::HashSet;

use rand::Rng;

use super::{
    generator::{append_one, OperationKind},
    types::{AbstractExecutor, Workload},
};

pub fn remove(workload: &Workload, index: usize) -> Option<Workload> {
    let mut ops = workload.ops.clone();
    ops.remove(index);
    let mut exec = AbstractExecutor::new();
    if !exec.replay(&Workload { ops }).is_ok() {
        None
    } else {
        Some(exec.recording)
    }
}

pub fn insert(
    rng: &mut impl Rng,
    workload: &Workload,
    index: usize,
    pick_from: HashSet<OperationKind>,
) -> Option<Workload> {
    let (before, after) = workload.ops.split_at(index);
    let mut exec = AbstractExecutor::new();
    if !exec
        .replay(&Workload {
            ops: before.to_vec(),
        })
        .is_ok()
    {
        return None;
    }
    append_one(rng, &mut exec, pick_from);
    if !exec
        .replay(&Workload {
            ops: after.to_vec(),
        })
        .is_ok()
    {
        None
    } else {
        Some(exec.recording)
    }
}

mod tests {
    use rand::{rngs::StdRng, SeedableRng};

    use crate::abstract_fs::{generator::generate_new, types::Operation};

    use super::*;

    #[test]
    fn test_remove() {
        let w = Workload {
            ops: vec![
                Operation::MKDIR {
                    path: "/foobar".to_owned(),
                    mode: vec![],
                },
                Operation::CREATE {
                    path: "/foobar/boo".to_owned(),
                    mode: vec![],
                },
                Operation::CREATE {
                    path: "/foobar/zoo".to_owned(),
                    mode: vec![],
                },
            ],
        };
        assert_eq!(None, remove(&w, 0));
        assert_eq!(
            Some(Workload {
                ops: vec![
                    Operation::MKDIR {
                        path: "/foobar".to_owned(),
                        mode: vec![],
                    },
                    Operation::CREATE {
                        path: "/foobar/zoo".to_owned(),
                        mode: vec![],
                    },
                ],
            }),
            remove(&w, 1)
        );
    }

    #[test]
    fn test_append() {
        let mut rng = StdRng::seed_from_u64(123);
        let w = Workload {
            ops: vec![
                Operation::MKDIR {
                    path: "/foobar".to_owned(),
                    mode: vec![],
                },
                Operation::CREATE {
                    path: "/foobar/boo".to_owned(),
                    mode: vec![],
                },
                Operation::REMOVE {
                    path: "/foobar/boo".to_owned(),
                },
            ],
        };
        assert_eq!(
            None,
            insert(&mut rng, &w, 1, HashSet::from([OperationKind::REMOVE]))
        );
        assert_eq!(
            Some(Workload {
                ops: vec![
                    Operation::MKDIR {
                        path: "/foobar".to_owned(),
                        mode: vec![],
                    },
                    Operation::CREATE {
                        path: "/foobar/boo".to_owned(),
                        mode: vec![],
                    },
                    Operation::REMOVE {
                        path: "/foobar/boo".to_owned(),
                    },
                    Operation::REMOVE {
                        path: "/foobar".to_owned(),
                    },
                ],
            }),
            insert(&mut rng, &w, 3, HashSet::from([OperationKind::REMOVE]))
        );
    }

    #[test]
    fn smoke_test_mutate() {
        let mut rng = StdRng::seed_from_u64(123);
        let mut w = generate_new(&mut rng, 3);
        for _ in 0..10000 {
            let p: f64 = rng.gen();
            if w.ops.is_empty() || p >= 0.5 {
                let index = rng.gen_range(0..=w.ops.len());
                if let Some(workload) = insert(&mut rng, &w, index, OperationKind::all()) {
                    w = workload;
                }
            } else {
                let index = rng.gen_range(0..w.ops.len());
                if let Some(workload) = remove(&w, index) {
                    w = workload;
                }
            }
        }
    }
}
