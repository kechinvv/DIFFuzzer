/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::cmp::max;

use super::{flags::Mode, node::FileDescriptorIndex, operation::Operation, workload::Workload};

/// Generates name of variable for the descriptor.
fn descriptor_to_var(des: &FileDescriptorIndex) -> String {
    format!("fd_{}", des.0)
}

impl Workload {
    /// Generates C code from workload, that can be run after building with executor.
    pub fn encode_c(&self) -> String {
        let mut result = String::new();
        result.push_str("#include \"executor.h\"\n");
        let mut descriptors_n = 0;
        for op in self.ops.iter() {
            if let Operation::Open { path: _, des } = op {
                descriptors_n = max(descriptors_n, des.0 + 1);
            }
        }
        if descriptors_n > 0 {
            let descriptors_vars: Vec<String> =
                (0..descriptors_n).map(|it| format!("fd_{}", it)).collect();
            result.push_str(format!("\nint {};\n\n", descriptors_vars.join(", ")).as_str());
        } else {
            result.push_str("\n// no descriptors\n\n");
        }
        result.push_str("void test_workload()\n");
        result.push_str("{\n");
        for op in &self.ops {
            match op {
                Operation::Create { path, mode } => {
                    result.push_str(
                        format!("do_create(\"{}\", {});\n", path, encode_mode(mode).as_str())
                            .as_str(),
                    );
                }
                Operation::MkDir { path, mode } => {
                    result.push_str(
                        format!("do_mkdir(\"{}\", {});\n", path, encode_mode(mode).as_str())
                            .as_str(),
                    );
                }
                Operation::Remove { path } => {
                    result.push_str(format!("do_remove(\"{}\");\n", path).as_str());
                }
                Operation::Hardlink { old_path, new_path } => {
                    result.push_str(
                        format!("do_hardlink(\"{}\", \"{}\");\n", old_path, new_path).as_str(),
                    );
                }
                Operation::Rename { old_path, new_path } => {
                    result.push_str(
                        format!("do_rename(\"{}\", \"{}\");\n", old_path, new_path).as_str(),
                    );
                }
                Operation::Open { path, des } => {
                    result.push_str(
                        format!("{} = do_open(\"{}\");\n", descriptor_to_var(des), path).as_str(),
                    );
                }
                Operation::Close { des } => {
                    result.push_str(format!("do_close({});\n", descriptor_to_var(des)).as_str());
                }
                Operation::Read { des, size } => {
                    result.push_str(
                        format!("do_read({}, {});\n", descriptor_to_var(des), size).as_str(),
                    );
                }
                Operation::Write {
                    des,
                    src_offset,
                    size,
                } => {
                    result.push_str(
                        format!(
                            "do_write({}, {}, {});\n",
                            descriptor_to_var(des),
                            src_offset,
                            size
                        )
                        .as_str(),
                    );
                }
                Operation::FSync { des } => {
                    result.push_str(format!("do_fsync({});\n", descriptor_to_var(des)).as_str());
                }
                Operation::Symlink { target, linkpath } => {
                    result.push_str(
                        format!("do_symlink(\"{}\", \"{}\");\n", target, linkpath).as_str(),
                    );
                }
            }
        }
        result.push('}');
        result
    }
}

fn encode_mode(mode: &Mode) -> String {
    if mode.is_empty() {
        0.to_string()
    } else {
        let mode_str: Vec<String> = mode.iter().map(|mf| mf.to_string()).collect();
        mode_str.join(" | ")
    }
}

#[cfg(test)]
mod tests {
    use crate::abstract_fs::{flags::ModeFlag, node::FileDescriptorIndex};

    use super::*;

    #[test]
    fn test_encode_c_empty() {
        let expected = r#"
#include "executor.h"

// no descriptors

void test_workload()
{
}
"#
        .trim();
        let actual = Workload { ops: vec![] }.encode_c();
        assert_eq!(expected, actual);
    }

    #[test]
    fn test_encode_c() {
        let expected = r#"
#include "executor.h"

int fd_0, fd_1;

void test_workload()
{
do_mkdir("/foo", 0);
do_create("/foo/bar", S_IRWXU | S_IRWXG | S_IROTH | S_IXOTH);
fd_0 = do_open("/foo/bar");
do_write(fd_0, 999, 1024);
do_close(fd_0);
do_hardlink("/foo/bar", "/baz");
fd_1 = do_open("/baz");
do_read(fd_1, 1024);
do_fsync(fd_1);
do_close(fd_1);
do_rename("/baz", "/gaz");
do_symlink("/foo", "/moo");
do_remove("/foo");
}
"#
        .trim();
        let mode = vec![
            ModeFlag::S_IRWXU,
            ModeFlag::S_IRWXG,
            ModeFlag::S_IROTH,
            ModeFlag::S_IXOTH,
        ];
        let actual = Workload {
            ops: vec![
                Operation::MkDir {
                    path: "/foo".into(),
                    mode: vec![],
                },
                Operation::Create {
                    path: "/foo/bar".into(),
                    mode: mode.clone(),
                },
                Operation::Open {
                    path: "/foo/bar".into(),
                    des: FileDescriptorIndex(0),
                },
                Operation::Write {
                    des: FileDescriptorIndex(0),
                    src_offset: 999,
                    size: 1024,
                },
                Operation::Close {
                    des: FileDescriptorIndex(0),
                },
                Operation::Hardlink {
                    old_path: "/foo/bar".into(),
                    new_path: "/baz".into(),
                },
                Operation::Open {
                    path: "/baz".into(),
                    des: FileDescriptorIndex(1),
                },
                Operation::Read {
                    des: FileDescriptorIndex(1),
                    size: 1024,
                },
                Operation::FSync {
                    des: FileDescriptorIndex(1),
                },
                Operation::Close {
                    des: FileDescriptorIndex(1),
                },
                Operation::Rename {
                    old_path: "/baz".into(),
                    new_path: "/gaz".into(),
                },
                Operation::Symlink {
                    target: "/foo".into(),
                    linkpath: "/moo".into(),
                },
                Operation::Remove {
                    path: "/foo".into(),
                },
            ],
        }
        .encode_c();
        assert_eq!(expected, actual);
    }
}
