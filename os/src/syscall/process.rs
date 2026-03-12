//! syscall: process

use crate::loader::get_app_data_by_name;
use crate::mm::page_table::copy_from_user_str;
use crate::task::{current_user_token, exit_current_and_run_next, suspend_current_and_run_next};
use crate::timer::get_time_us;
use crate::task::processor::current_task;
use crate::task::manager::enqueue_task;

pub fn sys_exit(xstate: i32) -> ! {
    println!("[kernel] Application exited with code {}", xstate);
    exit_current_and_run_next();
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

pub fn sys_exec(path: *const u8) -> isize {
    let token = current_user_token();
    let path = copy_from_user_str(token, path);
    if let Some(data) = get_app_data_by_name(&path) {
        let task = current_task().unwrap();
        task.exec(data);
        0
    } else {
        -1
    }
}
