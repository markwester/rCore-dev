mod context;
pub mod manager;
mod pid;
pub mod processor;
mod switch;
mod task;

use context::TaskContext;
use crate::loader::get_app_data_by_name;
use alloc::sync::Arc;
use lazy_static::lazy_static;
use manager::{TASK_MANAGER, enqueue_task};
use processor::{schedule, take_current_task};
use switch::__switch;
use task::{TaskControlBlock, TaskStatus};
pub use processor::{run_tasks, current_user_token, current_task, current_trap_cx};

pub fn mark_current_suspended() {
    TASK_MANAGER.exclusive_access().mark_current_suspended();
}

fn pick_next_task() -> *mut TaskContext {
    let task = current_task().unwrap();
    let mut task_inner = task.inner_exclusive_access();
    let task_ctx_ptr = &mut task_inner.task_cx as *mut TaskContext;

    drop(task_inner);

    enqueue_task(task);
    task_ctx_ptr
}

pub fn suspend_current_and_run_next() {
    mark_current_suspended();
    let next_task_ctx = pick_next_task();
    schedule(next_task_ctx);
}

/// mark zombie / save exit_code / push child in initproc / schedule
pub fn exit_current_and_run_next(exit_code: i32) {
    // take from Processor
    let task = take_current_task().unwrap();
    // **** access current TCB exclusively
    let mut inner = task.inner_exclusive_access();
    // Change status to Zombie
    inner.task_status = TaskStatus::Zombie;
    // Record exit code
    inner.exit_code = exit_code;
    // do not move to its parent but under initproc

    // ++++++ access initproc TCB exclusively
    {
        let mut initproc_inner = INITPROC.inner_exclusive_access();
        for child in inner.children.iter() {
            child.inner_exclusive_access().parent = Some(Arc::downgrade(&INITPROC));
            initproc_inner.children.push(child.clone());
        }
    }
    // ++++++ stop exclusively accessing parent PCB

    inner.children.clear();
    // deallocate user space
    inner.memory_set.recycle_data_pages();
    drop(inner);
    // **** stop exclusively accessing current PCB
    // drop task manually to maintain rc correctly
    drop(task);
    // we do not have to save task context
    let mut _unused = TaskContext::zeroed();
    schedule(&mut _unused as *mut _);
}

lazy_static! {
    pub static ref INITPROC: Arc<TaskControlBlock> = Arc::new(TaskControlBlock::new(
        get_app_data_by_name("initproc").unwrap()
    ));
}

pub fn add_initproc() {
    enqueue_task(INITPROC.clone());
}
