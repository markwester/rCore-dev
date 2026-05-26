//! syscall: process

use crate::fs::{OpenFlags, open_file};
use crate::mm::page_table::{copy_from_user_str, translated_refmut};
use crate::mm::translated_ref;
use crate::task::{current_process, current_user_token, exit_current_and_run_next, suspend_current_and_run_next};
use crate::timer::get_time_ms;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use crate::task::{SignalFlags, pid2pcb};

pub fn sys_exit(exit_code: i32) -> ! {
    exit_current_and_run_next(exit_code);
    panic!("Unreachable in sys_exit!");
}

pub fn sys_yield() -> isize {
    suspend_current_and_run_next();
    0
}

pub fn sys_get_time() -> isize {
    get_time_ms() as isize
}

pub fn sys_fork() -> isize {
    let pcb  = current_process();
    let child_pcb = pcb.fork();
    let child_pid = child_pcb.getpid();
    // do not need to add spec, because added it when current go trap_handler
    // set child syscall return value to 0(a0)
    let pcb_inner = child_pcb.inner_exclusive_access();
    let main_task = pcb_inner.tasks[0].as_ref().unwrap();
    let child_trap_ctx = main_task.inner_exclusive_access().get_trap_cx();
    child_trap_ctx.x[0] = 0;
    child_pid as isize
}

pub fn sys_exec(path: *const u8, mut args: *const usize) -> isize {
    let token = current_user_token();
    let path = copy_from_user_str(token, path);
    let mut args_v: Vec<String> = Vec::new();
    loop {
        let arg_ptr = *translated_ref(token, args);
        if arg_ptr == 0usize {
            break;
        }
        args_v.push(copy_from_user_str(token, arg_ptr as *const u8));
        unsafe {
            args = args.add(1);
        }
    }
    if let Some(data) = open_file(&path, OpenFlags::RDONLY) {
        let all_data = data.read_all();
        let argc = args_v.len();
        let pcb = current_process();
        pcb.exec(all_data.as_slice(), args_v);
        // return argc because cx.x[10] will be covered in trap_handler
        // if exec successed, the result will not meaningful and can be ignored
        // but if exec failed, the result will covered x[10] by trap_handler, it's meaningful for user
        // so we return argc and cover x[10] in trap_handler to promiss x[10] is correct argc
        argc as isize
    } else {
        -1
    }
}

/// If there is not a child process whose pid is same as given, return -1.
/// Else if there is a child process but it is still running, return -2.
pub fn sys_waitpid(pid: isize, exit_code_ptr: *mut i32) -> isize {
    let pcb = current_process();

    // ---- access current PCB exclusivel
    let mut inner = pcb.inner_exclusive_access();
    // if not child
    if inner
        .children
        .iter()
        .find(|p| pid == -1 || pid as usize == p.getpid())
        .is_none()
    {
        return -1;
        // ---- stop exclusively accessing current PCB
    }

    let pair = inner.children.iter().enumerate().find(|(_, p)| {
        // ++++ temporarily access child PCB exclusively
        p.inner_exclusive_access().is_zombie && (pid == -1 || pid as usize == p.getpid())
        // ++++ stop exclusively accessing child PCB
    });

    if let Some((idx, _)) = pair {
        let child = inner.children.remove(idx);
        // confirm that child will be deallocated after removing from children list
        assert_eq!(Arc::strong_count(&child), 1);
        let found_pid = child.getpid();
        // ++++ temporarily access child TCB exclusively
        let exit_code = child.inner_exclusive_access().exit_code;
        // ++++ stop exclusively accessing child PCB
        *translated_refmut(inner.memory_set.token(), exit_code_ptr) = exit_code;
        found_pid as isize
    } else {
        -2
    }
}

pub fn sys_getpid() -> isize {
    current_process().getpid() as isize
}

// fn check_sigaction_error(signal: SignalFlags, action: usize, old_action: usize) -> bool {
//     if action == 0
//         || old_action == 0
//         || signal == SignalFlags::SIGKILL
//         || signal == SignalFlags::SIGSTOP
//     {
//         true
//     } else {
//         false
//     }
// }

/// 功能：为当前进程设置某种信号的处理函数，同时保存设置之前的处理函数。
/// 参数：signum 表示信号的编号，action 表示要设置成的处理函数的指针
/// old_action 表示用于保存设置之前的处理函数的指针（SignalAction 结构稍后介绍）。
/// 返回值：如果传入参数错误（比如传入的 action 或 old_action 为空指针或者）
/// 信号类型不存在返回 -1 ，否则返回 0 。
/// syscall ID: 134
// pub fn sys_sigaction(
//     signum: i32,
//     action: *const SignalAction,
//     old_action: *mut SignalAction,
// ) -> isize {
//     let token = current_user_token();
//     let task = current_task().unwrap();
//     let mut inner = task.inner_exclusive_access();
//     if signum as usize > MAX_SIG {
//         return -1;
//     }
//     if let Some(flag) = SignalFlags::from_bits(1 << signum) {
//         if check_sigaction_error(flag, action as usize, old_action as usize) {
//             return -1;
//         }
//         let prev_action = inner.signal_actions.table[signum as usize];
//         *translated_refmut(token, old_action) = prev_action;
//         inner.signal_actions.table[signum as usize] = *translated_ref(token, action);
//         0
//     } else {
//         -1
//     }
// }

// pub fn sys_sigprocmask(mask: u32) -> isize {
//     if let Some(task) = current_task() {
//         let mut inner = task.inner_exclusive_access();
//         let old_mask = inner.signal_mask;
//         if let Some(flag) = SignalFlags::from_bits(mask as i32) {
//             inner.signal_mask = flag;
//             old_mask.bits() as isize
//         } else {
//             -1
//         }
//     } else {
//         -1
//     }
// }

pub fn sys_kill(pid: usize, signum: i32) -> isize {
    if let Some(pcb) = pid2pcb(pid) {
        if let Some(flag) = SignalFlags::from_bits(1 << signum) {
            // insert the signal if legal
            let mut pcb_inner = pcb.inner_exclusive_access();
            if pcb_inner.signals.contains(flag) {
                return -1;
            }
            pcb_inner.signals.insert(flag);
            0
        } else {
            -1
        }
    } else {
        -1
    }
}

// pub fn sys_sigreturn() -> isize {
//     let task = current_task().unwrap();
//     let mut inner = task.inner_exclusive_access();
//     inner.handling_sig = -1;
//     let trap_ctx_backup = inner.trap_ctx_backup.take();
//     let current_trap_ctx = inner.get_trap_cx();
//     *current_trap_ctx = trap_ctx_backup.unwrap();
//     current_trap_ctx.x[10] as isize
// }