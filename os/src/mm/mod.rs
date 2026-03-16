pub mod heap_allocator;
pub mod address;
pub mod page_table;
mod frame_allocator;
pub mod memory_set;
pub use memory_set::KERNEL_SPACE;
pub use memory_set::remap_test;

pub fn init() {
    heap_allocator::init_heap();
    frame_allocator::init_frame_allocator();
    KERNEL_SPACE.exclusive_access().activate();
}
