//! print call stack

#![no_std]
#![no_main]

#[macro_use]
extern crate user_lib;
use core::{arch::asm, ptr};

pub fn print_callstack() {
    unsafe {
        let mut fp: *const usize;
        asm!("mv {}, fp", out(reg) fp);
        loop {
            if fp == ptr::null() {
                break;
            }
            let saved_ra = *fp.sub(1);
            let saved_fp = *fp.sub(2);
            println!("ra: {:#x}, fp: {:#x}", saved_ra, saved_fp);
            fp = saved_fp as *const usize;
        }
    }
}

pub fn level1() {
    println!("level1");
    level2();
}

pub fn level2() {
    println!("level2");
    print_callstack();
}

#[unsafe(no_mangle)]
pub fn main() -> isize {
    println!("Test print_callstack!");
    level1();
    0
}
