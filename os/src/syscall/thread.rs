//! syscall for thread management

use crate::mm::kernel_token;
use crate::task::{current_task, TaskControlBlock, enqueue_task};
use alloc::sync::Arc;
use crate::trap::{TrapContext, trap_handler};

#[allow(unused)]
pub fn sys_thread_create(entry: usize, arg: usize) -> isize {
    let task = current_task().unwrap();
    let process = task.process.upgrade().unwrap();
    // create a new thread
    let new_task = Arc::new(TaskControlBlock::new(
        Arc::clone(&process),
        task.inner_exclusive_access().res.as_ref().unwrap().ustack_base,
        true,
    ));
    // add new task to scheduler
    enqueue_task(Arc::clone(&new_task));
    let new_task_inner = new_task.inner_exclusive_access();
    let new_task_res = new_task_inner.res.as_ref().unwrap();
    let new_task_tid = new_task_res.tid;
    let mut process_inner = process.inner_exclusive_access();
    // add new thread to current process
    let tasks = &mut process_inner.tasks;
    while tasks.len() < new_task_tid + 1 {
        tasks.push(None);
    }
    tasks[new_task_tid] = Some(Arc::clone(&new_task));
    let new_task_trap_cx = new_task_inner.get_trap_cx();
    *new_task_trap_cx = TrapContext::app_init_context(
        entry,
        new_task_res.get_ustack_top(),
        kernel_token(),
        new_task.kstack.get_top(),
        trap_handler as usize,
    );
    (*new_task_trap_cx).x[10] = arg;
    new_task_tid as isize
}

#[allow(unused)]
pub fn sys_waittid(tid: usize) -> i32 {
    let task = current_task().unwrap();
    let task_inner = task.inner_exclusive_access();
    let process = task.process.upgrade().unwrap();
    let mut process_inner = process.inner_exclusive_access();
    // a thread cannot wait for itself
    if task_inner.res.as_ref().unwrap().tid == tid {
        return -1;
    }

    let mut exit_code: Option<i32> = None;
    let wanted_task = process_inner.tasks[tid].as_ref();
    if let Some(wanted_task) = wanted_task {
        if let Some(wanted_exit_code) = wanted_task.inner_exclusive_access().exit_code {
            exit_code = Some(wanted_exit_code);
        }
    } else {
        // waited thread doesn't exist
        return -1;
    }

    if let Some(exit_code) = exit_code {
        process_inner.tasks[tid] = None;
        exit_code
    } else {
        // waited thread has not exited
        -2
    }
}