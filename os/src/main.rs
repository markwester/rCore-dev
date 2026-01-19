#![no_std]
#![no_main]
// #![deny(missing_docs)]
// #![deny(warnings)]

#[macro_use]
mod lang_items;
pub mod batch;
mod console;
mod sbi;
mod sync;
pub mod syscall;
pub mod trap;
mod loader;

use core::arch::global_asm;
global_asm!(include_str!("entry.asm"));
global_asm!(include_str!("link_app.S"));

// fn info(args: ) {
//     println!("\x1b[{}m {:?} \x1b[0m", 31, args);
// }

// fn warn(args: fmt::Alignments) {
//     println!("\x1b[{}m {:?} \x1b[0m", 93, args);
// }

#[unsafe(no_mangle)]
pub fn rust_main() -> ! {
    clear_bss();
    println!("\x1b[31m hello world! \x1b[0m");
    // panic!("Shutdown machine");
    trap::init();
    batch::init();
    batch::run_next_app();
}

fn clear_bss() {
    unsafe extern "C" {
        safe fn sbss();
        safe fn ebss();
    }

    (sbss as usize..ebss as usize).for_each(|a| unsafe {
        (a as *mut u8).write_volatile(0);
    });
}
