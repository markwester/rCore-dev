#![no_std]
#![no_main]
// #![deny(missing_docs)]
#![deny(warnings)]

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

use core::arch::global_asm;
global_asm!(include_str!("entry.asm"));
global_asm!(include_str!("link_app.S"));

fn clear_bss() {
    unsafe extern "C" {
        safe fn sbss();
        safe fn ebss();
    }

    (sbss as usize..ebss as usize).for_each(|a| unsafe {
        (a as *mut u8).write_volatile(0);
    });
}

#[unsafe(no_mangle)]
pub fn rust_main() -> ! {
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


