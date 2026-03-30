//! bitmap for block allocation

use crate::BLOCK_SZ;
use crate::block_cache::get_block_cache;
use crate::block_dev::BlockDevice;
use alloc::sync::Arc;

pub struct Bitmap {
    start_block_id: usize,
    blocks: usize,
}

// bitmap save to blocks
// blocks -> number of blocks
// [start_block_id, start_block_id + blocks)
impl Bitmap {
    pub fn new(start_block_id: usize, blocks: usize) -> Self {
        Self {
            start_block_id,
            blocks,
        }
    }

    pub fn maximum(&self) -> usize {
        self.blocks * BLOCK_SZ * 8
    }
}

type BitmapBlock = [u64; 64];

const BLOCK_BITS: usize = BLOCK_SZ * 8;

impl Bitmap {
    pub fn alloc(&self, block_device: &Arc<dyn BlockDevice>) -> Option<usize> {
        for block_id in 0..self.blocks {
            let pos = get_block_cache(self.start_block_id + block_id, Arc::clone(block_device))
                .lock()
                .modify(0, |bitmap_block: &mut BitmapBlock| {
                    if let Some((bits64_pos, inner_pos)) = bitmap_block
                        .iter()
                        .enumerate()
                        .find(|(_, bits64)| **bits64 != u64::MAX)
                        .map(|(bits64_pos, bits64)| (bits64_pos, bits64.trailing_ones() as usize))
                    {
                        bitmap_block[bits64_pos] |= 1u64 << inner_pos;
                        Some(block_id * BLOCK_SZ + bits64_pos * 64 + inner_pos as usize)
                    } else {
                        None
                    }
                });
            if pos.is_some() {
                return pos;
            }
        }
        None
    }

    pub fn dealloc(&self, block_device: &Arc<dyn BlockDevice>, pos: usize) {
        let (block_id, bits64_pos, inner_pos) = decode_pos(pos);
        get_block_cache(block_id + self.start_block_id, Arc::clone(block_device))
            .lock()
            .modify(0, |bitmap_block: &mut BitmapBlock| {
                assert!(bitmap_block[bits64_pos] & (1u64 << inner_pos) != 0);
                bitmap_block[bits64_pos] &= !(1u64 << inner_pos);
            });
    }
}

fn decode_pos(pos: usize) -> (usize, usize, usize) {
    let block_id = pos / BLOCK_BITS;
    let bits64_pos = pos % BLOCK_BITS / 64;
    let inner_pos = pos % BLOCK_BITS % 64;
    (block_id, bits64_pos, inner_pos)
}
