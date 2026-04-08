//! filesystem

mod inode;
mod stdio;
pub use inode::list_apps;
use crate::mm::page_table::UserBuffer;

pub use inode::open_file;
pub use inode::OpenFlags;
pub use stdio::{Stdin, Stdout};

#[allow(dead_code)]
pub trait File : Send + Sync {
    fn readable(&self) -> bool;
    fn writable(&self) -> bool;
    fn read(&self, buf: UserBuffer) -> usize;
    fn write(&self, buf: UserBuffer) -> usize;
}