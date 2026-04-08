//! block device driver

use lazy_static::*;
use alloc::sync::Arc;
use easy_fs::BlockDevice;
use crate::board::BlockDeviceImpl;


mod virtio_blk;
pub use virtio_blk::VirtIOBlock;

// #[cfg(feature = "board_k210")]
// type BlockDeviceImpl = sdcard::SDCardWrapper;

lazy_static! {
    pub static ref BLOCK_DEVICE: Arc<dyn BlockDevice> = Arc::new(BlockDeviceImpl::new());
}