//! print call stack

#![allow(unused)]

use core::{arch::asm, ptr};

/// 符号信息结构
struct Symbol {
    addr: usize,
    name: &'static str,
}

/// 已知的内核符号表（可以通过 nm 命令获取）
/// 使用方法：nm target/riscv64gc-unknown-none-elf/release/os | grep " T " | sort
const KNOWN_SYMBOLS: &[Symbol] = &[
    // Symbol { addr: 0x80200000, name: "_start" },
    // Symbol { addr: 0x80200000, name: "rust_main" },
    // 可以添加更多符号
];

/// 根据地址查找最接近的符号
fn find_symbol(addr: usize) -> Option<(&'static str, usize)> {
    let mut best_match: Option<(&'static str, usize)> = None;

    for symbol in KNOWN_SYMBOLS {
        if symbol.addr <= addr {
            let offset = addr - symbol.addr;
            match best_match {
                None => best_match = Some((symbol.name, offset)),
                Some((_, best_offset)) => {
                    if offset < best_offset {
                        best_match = Some((symbol.name, offset));
                    }
                }
            }
        }
    }

    best_match
}

pub fn print_callstack() {
    unsafe {
        let mut fp: *const usize;
        asm!("mv {}, fp", out(reg) fp);

        // 内核地址空间包括：
        // 1. 内核代码/数据段（包含 boot stack）：0x8000_0000 ~ 0x8800_0000
        //    - .text, .rodata, .data, .bss 都在这个范围
        //    - boot stack 在 .bss 段中
        // 2. 任务内核栈（高地址）：0xFFFF_FFFF_xxxx_xxxx（TRAMPOLINE 下方）
        //    - 每个任务有自己的内核栈，位于 TRAMPOLINE - app_id * (KERNEL_STACK_SIZE + PAGE_SIZE)
        const KERNEL_CODE_START: usize = 0x8000_0000;
        const KERNEL_CODE_END: usize = 0x8800_0000;
        const KERNEL_STACK_START: usize = 0xFFFF_FFFF_0000_0000;

        loop {
            // 检查 fp 是否为空
            if fp == ptr::null() {
                break;
            }

            let fp_addr = fp as usize;

            // 检查 fp 是否在内核地址空间
            // 内核栈在高地址或内核代码段
            let is_kernel_addr = (fp_addr >= KERNEL_STACK_START)
                || (fp_addr >= KERNEL_CODE_START && fp_addr < KERNEL_CODE_END);

            if !is_kernel_addr {
                break;
            }

            // 检查是否可以安全地访问 fp-1 和 fp-2
            if fp_addr < 16 {
                break;
            }

            let saved_ra = *fp.sub(1);
            let saved_fp = *fp.sub(2);

            // 尝试查找符号
            if let Some((symbol_name, offset)) = find_symbol(saved_ra) {
                println!(
                    "ra: {:#x} ({}+{:#x}), fp: {:#x}",
                    saved_ra, symbol_name, offset, saved_fp
                );
            } else {
                println!("ra: {:#x}, fp: {:#x}", saved_ra, saved_fp);
            }

            // 检查 saved_fp 是否在内核地址空间
            let next_is_kernel = (saved_fp >= KERNEL_STACK_START)
                || (saved_fp >= KERNEL_CODE_START && saved_fp < KERNEL_CODE_END);

            if !next_is_kernel {
                break;
            }

            fp = saved_fp as *const usize;
        }
    }
}
