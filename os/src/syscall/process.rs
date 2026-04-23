//! syscall: process

use crate::fs::{OpenFlags, open_file};
use crate::mm::page_table::{copy_from_user_str, translated_refmut};
use crate::mm::translated_ref;
use crate::task::manager::enqueue_task;
use crate::task::processor::current_task;
use crate::task::{current_user_token, exit_current_and_run_next, suspend_current_and_run_next};
use crate::timer::get_time_us;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;

pub fn sys_exit(exit_code: i32) -> ! {
    exit_current_and_run_next(exit_code);
    panic!("Unreachable in sys_exit!");
}

pub fn sys_yield() -> isize {
    suspend_current_and_run_next();
    0
}

pub fn sys_get_time() -> isize {
    get_time_us() as isize
}

pub fn sys_fork() -> isize {
    let current_task = current_task().unwrap();
    let child_tcb = current_task.fork();
    let child_pid = child_tcb.pid.0;
    // do not need to add spec, because added it when current go trap_handler
    // set child syscall return value to 0(a0)
    let child_trap_ctx = child_tcb.inner_exclusive_access().get_trap_cx();
    child_trap_ctx.x[0] = 0;

    enqueue_task(child_tcb);
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
        let task = current_task().unwrap();
        let all_data = data.read_all();
        task.exec(all_data.as_slice(), args_v);
        0
    } else {
        -1
    }
}

/// If there is not a child process whose pid is same as given, return -1.
/// Else if there is a child process but it is still running, return -2.
pub fn sys_waitpid(pid: isize, exit_code_ptr: *mut i32) -> isize {
    let task = current_task().unwrap();

    // ---- access current TCB exclusivel
    let mut inner = task.inner_exclusive_access();
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
        p.inner_exclusive_access().is_zombie() && (pid == -1 || pid as usize == p.getpid())
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
    current_task().unwrap().pid.0 as isize
}
