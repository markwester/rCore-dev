//! print call stack

#![allow(unused)]

use core::{arch::asm, ptr};

// 符号表：build.rs 从 `nm <kernel-elf>` 生成，`&[(addr, demangled-name)]`，
// 按 addr 升序，覆盖所有 .text 段符号。
include!(concat!(env!("OUT_DIR"), "/symbols.rs"));

// 行号表：build.rs 从 DWARF 调试信息生成，三张地址升序的并行数组 +
// 文件名表。无调试信息时为空表（所有帧的文件名:行号回退为 `?`，不会崩溃）。
include!(concat!(env!("OUT_DIR"), "/lines.rs"));

/// 根据地址查找最接近的符号：返回地址 <= `addr` 的最大符号及其偏移。
/// `SYMBOLS` 按 addr 升序排列，因此从后向前找第一个 addr <= 目标即可。
fn find_symbol(addr: usize) -> Option<(&'static str, usize)> {
    SYMBOLS
        .iter()
        .rev()
        .find(|(sym_addr, _)| *sym_addr <= addr)
        .map(|(sym_addr, name)| (*name, addr - sym_addr))
}

/// 根据地址查找其源码位置：返回地址 <= `addr` 的最大行表条目对应的
/// `(文件名, 行号)`。`LINE_ADDRS` 按 addr 升序，用二分（partition_point）
/// 取「不大于目标的最大地址」条目。空表 / 查不到 / 文件为 unknown 时返回 `None`，
/// 调用方用 `?` 回退。
fn find_line(addr: usize) -> Option<(&'static str, u32)> {
    if LINE_ADDRS.is_empty() || addr > u32::MAX as usize {
        return None;
    }
    // partition_point 的谓词为 `*a <= addr`，返回值为「<= addr 的元素个数」，
    // 即「最大的 <= addr 的元素」之后的下标；为 0 表示没有 <= addr 的条目。
    let idx = LINE_ADDRS.partition_point(|a| (*a as usize) <= addr);
    if idx == 0 {
        return None;
    }
    let i = idx - 1;
    let file_id = LINE_FILES[i];
    if file_id == 0 {
        return None; // "<unknown>" 哨兵
    }
    let line = LINE_LINES[i];
    if line == 0 {
        return None;
    }
    Some((LINE_FILE_TABLE[file_id as usize], line as u32))
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

            // 同时查符号表（函数名 + 偏移）和行号表（文件名:行号）。
            // 找不到行号时退化为 `?`，找不到符号时退化为裸地址，均不崩溃。
            let sym = find_symbol(saved_ra);
            let line = find_line(saved_ra);
            match (sym, line) {
                (Some((name, off)), Some((file, ln))) => {
                    println!(
                        "ra: {:#x} ({}+{:#x} at {}:{}), fp: {:#x}",
                        saved_ra, name, off, file, ln, saved_fp
                    );
                }
                (Some((name, off)), None) => {
                    println!(
                        "ra: {:#x} ({}+{:#x} at ?), fp: {:#x}",
                        saved_ra, name, off, saved_fp
                    );
                }
                (None, Some((file, ln))) => {
                    println!(
                        "ra: {:#x} ({}:{}), fp: {:#x}",
                        saved_ra, file, ln, saved_fp
                    );
                }
                (None, None) => {
                    println!("ra: {:#x}, fp: {:#x}", saved_ra, saved_fp);
                }
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
