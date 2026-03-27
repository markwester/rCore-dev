//! fs layout

use crate::BLOCK_SZ;
use crate::block_cache::get_block_cache;
use crate::block_dev::BlockDevice;
use alloc::sync::Arc;
use alloc::vec::Vec;

/// Magic number for sanity check
const EFS_MAGIC: u32 = 0x3b800001;

pub struct SuperBlock {
    magic: u32,
    pub total_blocks: u32,
    pub inode_bitmap_blocks: u32,
    pub inode_area_blocks: u32,
    pub data_bitmap_blocks: u32,
    pub data_area_blocks: u32,
}

impl SuperBlock {
    pub fn init(
        &mut self,
        total_blocks: u32,
        inode_bitmap_blocks: u32,
        inode_area_blocks: u32,
        data_bitmap_blocks: u32,
        data_area_blocks: u32,
    ) {
        *self = Self {
            magic: EFS_MAGIC,
            total_blocks,
            inode_bitmap_blocks,
            inode_area_blocks,
            data_bitmap_blocks,
            data_area_blocks,
        }
    }
    pub fn is_valid(&self) -> bool {
        self.magic == EFS_MAGIC
    }
}

const INODE_DIRECT_COUNT: usize = 28;
const INODE_INDIRECT1_COUNT: usize = BLOCK_SZ / 4;
const INODE_INDIRECT2_COUNT: usize = INODE_INDIRECT1_COUNT * BLOCK_SZ / 4;
const INODE_DIRECT_BOUND: usize = INODE_DIRECT_COUNT;
const INODE_INDIRECT1_BOUND: usize = INODE_DIRECT_BOUND + INODE_INDIRECT1_COUNT;
const INODE_INDIRECT2_BOUND: usize = INODE_INDIRECT1_BOUND + INODE_INDIRECT2_COUNT;
// direct can be save INODE_DIRECT_COUNT blocks
// indirect1 -> BLOCK_SZ / 4 blocks
// indirect2 -> BLOCK_SZ / 4 * BLOCK_SZ / 4 blocks
// save disk block id
#[repr(C)]
pub struct DiskInode {
    pub size: u32,
    pub direct: [u32; INODE_DIRECT_COUNT],
    pub indirect1: u32,
    pub indirect2: u32,
    type_: DiskInodeType,
}

#[derive(PartialEq)]
pub enum DiskInodeType {
    File,
    Directory,
}

type IndirectBlock = [u32; BLOCK_SZ / 4];

impl DiskInode {
    /// indirect1 and indirect2 block are allocated only when they are needed.
    pub fn init(&mut self, type_: DiskInodeType) {
        self.size = 0;
        self.direct.iter_mut().for_each(|v| *v = 0);
        self.indirect1 = 0;
        self.indirect2 = 0;
        self.type_ = type_;
    }
    pub fn is_dir(&self) -> bool {
        self.type_ == DiskInodeType::Directory
    }
    pub fn is_file(&self) -> bool {
        self.type_ == DiskInodeType::File
    }

    // get block id in disk
    pub fn get_block_id(&self, inner_id: u32, block_device: &Arc<dyn BlockDevice>) -> u32 {
        let inner_id = inner_id as usize;
        if inner_id < INODE_DIRECT_COUNT {
            self.direct[inner_id]
        } else if inner_id < INODE_INDIRECT1_BOUND {
            let indirect1_id = inner_id - INODE_DIRECT_BOUND;
            get_block_cache(self.indirect1 as usize, Arc::clone(block_device))
                .lock()
                .read(0, |indirect_blk: &IndirectBlock| indirect_blk[indirect1_id])
        } else {
            assert!(inner_id < INODE_INDIRECT2_BOUND);
            let indirect2_id = inner_id - INODE_INDIRECT2_BOUND;
            let inner_id1 = indirect2_id / INODE_INDIRECT1_COUNT;
            let inner_id2 = indirect2_id % INODE_INDIRECT1_COUNT;
            // get indirect1 block id
            let indirect1_blk = get_block_cache(self.indirect2 as usize, Arc::clone(block_device))
                .lock()
                .read(0, |indirect2_blk: &IndirectBlock| indirect2_blk[inner_id1]);
            get_block_cache(indirect1_blk as usize, Arc::clone(block_device))
                .lock()
                .read(0, |indirect1_blk: &IndirectBlock| indirect1_blk[inner_id2])
        }
    }

    /// cal data blocks needed for this size.
    pub fn data_blocks(&self) -> u32 {
        Self::_data_blocks(self.size)
    }
    fn _data_blocks(size: u32) -> u32 {
        // ceil(size / BLOCK_SZ)
        (size + BLOCK_SZ as u32 - 1) / BLOCK_SZ as u32
    }
    /// Return number of blocks needed include indirect1/2.
    pub fn total_blocks(size: u32) -> u32 {
        let data_blocks = Self::_data_blocks(size) as usize;
        let mut total = data_blocks as usize;
        // indirect1
        if data_blocks > INODE_DIRECT_COUNT {
            total += 1;
        }
        // indirect2
        if data_blocks > INODE_INDIRECT1_BOUND {
            total += 1;
            // sub indirect1
            total += (data_blocks - INODE_INDIRECT1_BOUND + INODE_INDIRECT1_COUNT - 1) / INODE_INDIRECT1_COUNT;
        }
        total as u32
    }
    pub fn blocks_num_needed(&self, new_size: u32) -> u32 {
        assert!(new_size >= self.size);
        Self::total_blocks(new_size) - Self::total_blocks(self.size)
    }

    pub fn increase_size(
        &mut self,
        new_size: u32,
        new_blocks: Vec<u32>,
        block_device: &Arc<dyn BlockDevice>,
    ) {

    }
}
