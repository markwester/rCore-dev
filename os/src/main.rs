//! main entrypoint for rCore

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
mod task;
pub mod config;
mod timer;
pub mod mm;
pub mod unwind;
mod fs;
mod drivers;

use core::arch::global_asm;
// use crate::board::BlockDeviceImpl;

extern crate alloc;
#[macro_use]
extern crate bitflags;

global_asm!(include_str!("entry.asm"));

#[path = "boards/qemu.rs"]
mod board;

fn clear_bss() {
    unsafe extern "C" {
        safe fn sbss();
        safe fn ebss();
    }
    unsafe {
        core::slice::from_raw_parts_mut(sbss as *const () as usize as *mut u8, ebss as *const () as usize - sbss as *const () as usize)
            .fill(0);
    }
}

const RCORE_LOGO: &str = r#"
 _______________________________
<  rCore: Rust-based RISC-V OS >
 -------------------------------
        \   ^__^
         \  (oo)\_______
            (__)\       )\/\
                ||----w |
                ||     ||

"#;

#[unsafe(no_mangle)]
pub fn rust_main() -> ! {
    clear_bss();
    println!("{}", RCORE_LOGO);
    mm::init();
    mm::remap_test();
    trap::init();
    // BlockDeviceImpl::new();
    println!("added initproc!");
    trap::enable_timer_interrupt();
    timer::set_next_tick();
    fs::list_apps();
    task::add_initproc();
    task::run_tasks();
    panic!("Unreachable in rust_main!");
}
