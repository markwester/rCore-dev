mod context;
mod switch;
mod task;
mod pid;
mod manager;
mod processor;

// use super::loader::get_app_data;
// use super::loader::get_num_app;
// use super::sync::UPSafeCell;
// use crate::sbi::shutdown;
// use crate::trap::context::TrapContext;
// use alloc::vec::Vec;
use context::TaskContext;
// use core::panic;
use lazy_static::lazy_static;
use switch::__switch;
use task::{TaskControlBlock, TaskStatus};
use alloc::sync::Arc;
use crate::{loader::get_app_data_by_name, task::processor::current_task};
use manager::{TASK_MANAGER, enqueue_task};
use processor::schedule;


pub fn mark_current_suspended() {
    TASK_MANAGER.exclusive_access().mark_current_suspended();
}

// pub fn mark_current_exited() {
//     TASK_MANAGER.mark_current_exited();
// }

// pub fn run_next_task() {
//     TASK_MANAGER.run_next_task();
// }

// pub fn run_first_task() {
//     TASK_MANAGER.run_first_task();
// }

pub fn suspend_current_and_run_next() {
    mark_current_suspended();
    let task = current_task().unwrap();
    let mut task_inner = task.inner_exclusive_access();
    let task_ctx_ptr = &mut task_inner.task_cx as *mut TaskContext;

    drop(task_inner);

    enqueue_task(task);
    schedule(task_ctx_ptr);
}

// pub fn exit_current_and_run_next() {
//     mark_current_exited();
//     run_next_task();
// }

// pub fn current_user_token() -> usize {
//     TASK_MANAGER.get_current_token()
// }

// pub fn current_trap_cx() -> &'static mut TrapContext {
//     TASK_MANAGER.get_current_trap_cx()
// }

lazy_static! {
    pub static ref INITPROC: Arc<TaskControlBlock> = Arc::new(
        TaskControlBlock::new(get_app_data_by_name("initproc").unwrap())
    );
}

pub fn add_initproc() {
    enqueue_task(INITPROC.clone());
}