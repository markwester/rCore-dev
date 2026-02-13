#![no_std]
#![no_main]
// #![deny(missing_docs)]
#![deny(warnings)]
#![feature(alloc_error_handler)]

#[macro_use]
mod console;
mod lang_items;
mod sbi;
mod sync;
pub mod syscall;
pub mod trap;
mod loader;
mod task;
pub mod config;
mod timer;
pub mod mm;

use core::arch::global_asm;
use mm::heap_allocator::init_heap;
extern crate alloc;
#[macro_use]
extern crate bitflags;

global_asm!(include_str!("entry.asm"));
global_asm!(include_str!("link_app.S"));

unsafe extern "C" {
    safe fn sbss();
    safe fn ebss();
}

fn clear_bss() {
    (sbss as usize..ebss as usize).for_each(|a| unsafe {
        (a as *mut u8).write_volatile(0);
    });
}

#[unsafe(no_mangle)]
pub fn rust_main() -> ! {
    init_heap();
    heap_test();
    clear_bss();
    println!("\x1b[31m hello rCore! \x1b[0m");
    trap::init();
    loader::init();
    loader::load_apps();
    trap::enable_timer_interrupt();
    timer::set_next_tick();
    task::run_first_task();
    panic!("Unreachable in rust_main!");
}

#[allow(unused)]
pub fn heap_test() {
    use alloc::boxed::Box;
    use alloc::vec::Vec;
    let bss_range = sbss as usize..ebss as usize;
    let a = Box::new(5);
    assert_eq!(*a, 5);
    assert!(bss_range.contains(&(a.as_ref() as *const _ as usize)));
    drop(a);
    let mut v: Vec<usize> = Vec::new();
    for i in 0..500 {
        v.push(i);
    }
    for i in 0..500 {
        assert_eq!(v[i], i);
    }
    assert!(bss_range.contains(&(v.as_ptr() as usize)));
    drop(v);
    println!("heap_test passed!");
}
