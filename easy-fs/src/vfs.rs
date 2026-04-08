//! vfs: virtual file system for easy-fs

use crate::block_cache::get_block_cache;
use crate::block_dev::BlockDevice;
use crate::efs::EasyFileSystem;
use crate::layout::{DIRENTRY_SZ, DirEntry, DiskInode, DiskInodeType};
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use spin::{Mutex, MutexGuard};

/// DiskInode 放在磁盘块中比较固定的位置，而 Inode 放在内存中，其成员表达的是DiskInode在磁盘中的位置信息。
pub struct Inode {
    block_id: usize,
    block_offset: usize,
    fs: Arc<Mutex<EasyFileSystem>>,
    block_device: Arc<dyn BlockDevice>,
}

impl Inode {
    pub fn new(
        block_id: usize,
        block_offset: usize,
        fs: Arc<Mutex<EasyFileSystem>>,
        block_device: Arc<dyn BlockDevice>,
    ) -> Self {
        Self {
            block_id,
            block_offset,
            fs,
            block_device,
        }
    }

    fn read_disk_inode<V>(&self, f: impl FnOnce(&DiskInode) -> V) -> V {
        get_block_cache(self.block_id, Arc::clone(&self.block_device))
            .lock()
            .read(self.block_offset, f)
    }

    fn modify_disk_inode<V>(&self, f: impl FnOnce(&mut DiskInode) -> V) -> V {
        get_block_cache(self.block_id, Arc::clone(&self.block_device))
            .lock()
            .modify(self.block_offset, f)
    }

    /// find inode id for name in the dir
    /// need to hold fs lock before call it
    fn find_inode_id(&self, name: &str, disk_inode: &DiskInode) -> Option<u32> {
        assert!(disk_inode.is_dir());
        let file_cnt = disk_inode.size as usize / DIRENTRY_SZ;
        let mut direntry = DirEntry::empty();
        for i in 0..file_cnt {
            disk_inode.read_at(i * DIRENTRY_SZ, direntry.as_bytes_mut(), &self.block_device);
            if direntry.name() == name {
                return Some(direntry.inode_number());
            }
        }
        None
    }

    /// find inode id for name and return inode obj
    /// need to hold fs lock before call it
    pub fn find(&self, name: &str) -> Option<Arc<Inode>> {
        let fs = self.fs.lock();
        self.read_disk_inode(|disk_inode| {
            self.find_inode_id(name, disk_inode).map(|inode_id| {
                let (block_id, block_offset) = fs.get_disk_inode_pos(inode_id);
                Arc::new(Self::new(
                    block_id as usize,
                    block_offset,
                    Arc::clone(&self.fs),
                    Arc::clone(&self.block_device),
                ))
            })
        })
    }

    pub fn ls(&self) -> Vec<String> {
        let _fs = self.fs.lock();
        self.read_disk_inode(|disk_inode| {
            assert!(disk_inode.is_dir());
            let file_cnt = disk_inode.size as usize / DIRENTRY_SZ;
            let mut direntry = DirEntry::empty();
            let mut res = Vec::new();
            for i in 0..file_cnt {
                assert_eq!(
                    disk_inode.read_at(
                        i * DIRENTRY_SZ,
                        direntry.as_bytes_mut(),
                        &self.block_device
                    ),
                    DIRENTRY_SZ
                );
                res.push(String::from(direntry.name()));
            }
            res
        })
    }

    /// Increase the size of a disk inode
    fn increase_size(
        &self,
        new_size: u32,
        disk_inode: &mut DiskInode,
        fs: &mut MutexGuard<EasyFileSystem>,
    ) {
        if new_size <= disk_inode.size {
            return;
        }
        let blocks_needed = disk_inode.blocks_num_needed(new_size);
        let mut v: Vec<u32> = Vec::new();
        for _ in 0..blocks_needed {
            v.push(fs.alloc_data());
        }
        disk_inode.increase_size(new_size, v, &fs.block_device);
    }

    pub fn create(&self, name: &str) -> Option<Arc<Inode>> {
        // check if name exists
        let mut fs = self.fs.lock();
        if self
            .read_disk_inode(|disk_inode| {
                assert!(disk_inode.is_dir());
                self.find_inode_id(name, disk_inode)
            })
            .is_some()
        {
            return None;
        }
        // create an new file
        // alloc inode
        let new_inode_id = fs.alloc_inode();
        let (new_inode_block_id, new_inode_block_offset) = fs.get_disk_inode_pos(new_inode_id);
        get_block_cache(new_inode_block_id as usize, Arc::clone(&self.block_device))
            .lock()
            .modify(new_inode_block_offset, |disk_inode: &mut DiskInode| {
                disk_inode.init(DiskInodeType::File);
            });
        // add direntry to parent dir
        self.modify_disk_inode(|disk_inode| {
            // append file in the direntry
            let file_cnt = disk_inode.size as usize / DIRENTRY_SZ;
            // increase size
            let new_size = disk_inode.size + DIRENTRY_SZ as u32;
            self.increase_size(new_size, disk_inode, &mut fs);
            // write direntry
            let direntry = DirEntry::new(name, new_inode_id);
            disk_inode.write_at(
                file_cnt * DIRENTRY_SZ,
                direntry.as_bytes(),
                &self.block_device,
            );
        });
        // new inode obj
        let (block_id, block_offset) = fs.get_disk_inode_pos(new_inode_id);
        Some(Arc::new(Self::new(
            block_id as usize,
            block_offset,
            Arc::clone(&self.fs),
            Arc::clone(&self.block_device),
        )))
        // release efs lock automatically by compiler
    }

    pub fn read_at(&self, offset: usize, buf: &mut [u8]) -> usize {
        let _fs = self.fs.lock();
        self.read_disk_inode(|disk_inode| disk_inode.read_at(offset, buf, &self.block_device))
    }

    pub fn write_at(&self, offset: usize, buf: &[u8]) -> usize {
        let mut fs = self.fs.lock();
        self.modify_disk_inode(|disk_inode| {
            self.increase_size((offset + buf.len()) as u32, disk_inode, &mut fs);
            disk_inode.write_at(offset, buf, &self.block_device)
        })
    }

    pub fn clear(&self) {
        let mut fs = self.fs.lock();
        self.modify_disk_inode(|disk_inode| {
            // dealloc data blocks
            let inode_size = disk_inode.size;
            let need_clean_blks = disk_inode.clear_size(&self.block_device);
            assert_eq!(need_clean_blks.len() as u32, DiskInode::total_blocks(inode_size));
            for blk in need_clean_blks {
                fs.dealloc_data(blk);
            }
        });
    }
}
