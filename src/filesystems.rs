use crate::mount::{btrfs::Btrfs, ext4::Ext4, f2fs::F2FS, mount::FileSystemMount};

pub const FILESYSTEMS: &[&dyn FileSystemMount] = &[&Ext4 {}, &Btrfs {}, &F2FS {}];

impl TryFrom<String> for &'static dyn FileSystemMount {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        for fs in FILESYSTEMS {
            if fs.to_string() == value {
                return Ok(*fs);
            }
        }
        Err(format!("unknown filesystem '{}'", value))
    }
}
