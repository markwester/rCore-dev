pub mod heap_allocator;
pub mod address;
pub mod page_table;
mod frame_allocator;
pub mod memory_set;
pub use memory_set::remap_test;

pub use address::{PhysAddr, PhysPageNum, StepByOne, VirtAddr, VirtPageNum};
pub use frame_allocator::{FrameTracker, frame_alloc, frame_dealloc};
pub use memory_set::{KERNEL_SPACE, MapPermission, MemorySet, kernel_token};
pub use page_table::{
    PageTable, PageTableEntry, UserBuffer, UserBufferIterator, translated_byte_buffer,
    translated_ref, translated_refmut, translated_str,
};

pub fn init() {
    heap_allocator::init_heap();
    frame_allocator::init_frame_allocator();
    KERNEL_SPACE.exclusive_access().activate();
}
