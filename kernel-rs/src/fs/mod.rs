//! Filesystem facade: tmpfs mounted at "/" plus the initrd unpacker.

pub mod initrd;
pub mod tmpfs;

/// Re-export the tmpfs implementation under the `vfs` name callers use.
pub mod vfs {
    pub use super::tmpfs::*;
}
