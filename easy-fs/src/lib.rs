//! lib

#![no_std]
#![allow(unused)]

mod block_dev;
mod block_cache;
mod layout;
mod bitmap;

extern crate alloc;

pub const BLOCK_SZ: usize = 512;