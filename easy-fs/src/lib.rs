//! lib

#![no_std]
#![allow(unused)]

mod block_dev;
mod block_cache;
mod layout;
mod bitmap;
mod efs;
mod vfs;

extern crate alloc;

pub const BLOCK_SZ: usize = 512;

// 公开导出关键类型
pub use block_dev::BlockDevice;
pub use efs::EasyFileSystem;
pub use vfs::Inode;