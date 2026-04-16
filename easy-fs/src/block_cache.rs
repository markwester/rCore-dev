//! block cache

use core::panic;

use crate::BLOCK_SZ;
use crate::block_dev::BlockDevice;
use alloc::collections::VecDeque;
use alloc::sync::Arc;
use lazy_static::lazy_static;
use spin::Mutex;
use core::mem::ManuallyDrop;
use alloc::boxed::Box;
use core::alloc::Layout;
use core::slice;

/// Use `ManuallyDrop` to ensure data is deallocated with an alignment of `BLOCK_SZ`
struct CacheData(ManuallyDrop<Box<[u8; BLOCK_SZ]>>);

impl CacheData {
    pub fn new() -> Self {
        let data = unsafe {
            let raw = alloc::alloc::alloc(Self::layout());
            Box::from_raw(raw as *mut [u8; BLOCK_SZ])
        };
        Self(ManuallyDrop::new(data))
    }

    fn layout() -> Layout {
        Layout::from_size_align(BLOCK_SZ, BLOCK_SZ).unwrap()
    }
}

impl Drop for CacheData {
    fn drop(&mut self) {
        let ptr = self.0.as_mut_ptr();
        unsafe { alloc::alloc::dealloc(ptr, Self::layout()) };
    }
}

impl AsRef<[u8]> for CacheData {
    fn as_ref(&self) -> &[u8] {
        let ptr = self.0.as_ptr() as *const u8;
        unsafe { slice::from_raw_parts(ptr, BLOCK_SZ) }
    }
}

impl AsMut<[u8]> for CacheData {
    fn as_mut(&mut self) -> &mut [u8] {
        let ptr = self.0.as_mut_ptr() as *mut u8;
        unsafe { slice::from_raw_parts_mut(ptr, BLOCK_SZ) }
    }
}

pub struct BlockCache {
    data: CacheData,
    block_id: usize,
    block_device: Arc<dyn BlockDevice>,
    modified: bool,
}

impl BlockCache {
    /// Load a new BlockCache from disk.
    pub fn new(block_id: usize, block_device: Arc<dyn BlockDevice>) -> Self {
        let mut data = CacheData::new();
        block_device.read_block(block_id, data.as_mut());
        Self {
            data,
            block_id,
            block_device,
            modified: false,
        }
    }

    pub fn sync(&mut self) {
        if self.modified {
            self.modified = false;
            self.block_device.write_block(self.block_id, self.data.as_mut());
        }
    }

    fn addr_of_offset(&self, offset: usize) -> usize {
        &self.data.as_ref()[offset] as *const _ as usize
    }

    pub fn get_ref<T>(&self, offset: usize) -> &T
    where
        T: Sized,
    {
        let type_size = core::mem::size_of::<T>();
        assert!(offset + type_size <= BLOCK_SZ);
        let addr = self.addr_of_offset(offset);
        unsafe { &*(addr as *const T) }
    }

    pub fn get_mut<T>(&mut self, offset: usize) -> &mut T
    where
        T: Sized,
    {
        let type_size = core::mem::size_of::<T>();
        assert!(offset + type_size <= BLOCK_SZ);
        self.modified = true;
        let addr = self.addr_of_offset(offset);
        unsafe { &mut *(addr as *mut T) }
    }

    pub fn read<T, V>(&self, offset: usize, f: impl FnOnce(&T) -> V) -> V {
        f(self.get_ref(offset))
    }

    pub fn modify<T, V>(&mut self, offset: usize, f: impl FnOnce(&mut T) -> V) -> V {
        f(self.get_mut(offset))
    }
}

impl Drop for BlockCache {
    fn drop(&mut self) {
        self.sync()
    }
}

const BLOCK_CACHE_SIZE: usize = 16;

pub struct BlockCacheManager {
    queue: VecDeque<(usize, Arc<Mutex<BlockCache>>)>,
}

impl BlockCacheManager {
    pub fn new() -> Self {
        Self {
            queue: VecDeque::new(),
        }
    }

    pub fn get_block_cache(
        &mut self,
        block_id: usize,
        block_device: Arc<dyn BlockDevice>,
    ) -> Arc<Mutex<BlockCache>> {
        if let Some(pair) = self.queue.iter().find(|pair| pair.0 == block_id) {
            Arc::clone(&pair.1)
        } else {
            // subtitue the oldest
            if self.queue.len() == BLOCK_CACHE_SIZE {
                if let Some((idx, _)) = self
                    .queue
                    .iter()
                    .enumerate()
                    .find(|(_, pair)| Arc::strong_count(&pair.1) == 1)
                {
                    self.queue.drain(idx..=idx);
                } else {
                    panic!("All BlockCache are in use!")
                }
            }
            // load new block in cache
            let block_cache = Arc::new(Mutex::new(BlockCache::new(block_id, block_device)));
            self.queue.push_back((block_id, Arc::clone(&block_cache)));
            block_cache
        }
    }

    pub fn sync_all(&mut self) {
        for (_, cache) in self.queue.iter() {
            cache.lock().sync();
        }
    }
}

lazy_static! {
    pub static ref BLOCK_CACHE_MANAGER: Mutex<BlockCacheManager> =
        Mutex::new(BlockCacheManager::new());
}

pub fn get_block_cache(
    block_id: usize,
    block_device: Arc<dyn BlockDevice>,
) -> Arc<Mutex<BlockCache>> {
    BLOCK_CACHE_MANAGER
        .lock()
        .get_block_cache(block_id, block_device)
}

pub fn sync_all_block_cache() {
    BLOCK_CACHE_MANAGER.lock().sync_all();
}
