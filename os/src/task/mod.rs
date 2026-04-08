mod context;
pub mod manager;
mod pid;
pub mod processor;
mod switch;
mod task;

use crate::fs::{OpenFlags, open_file};
use crate::sbi::shutdown;
use alloc::sync::Arc;
use context::TaskContext;
use lazy_static::lazy_static;
use manager::enqueue_task;
pub use processor::{current_task, current_trap_cx, current_user_token, run_tasks};
use processor::{schedule, take_current_task};
use switch::__switch;
use task::{TaskControlBlock, TaskStatus};

pub fn suspend_current_and_run_next() {
    // mark current task as suspended and enqueue it
    let task = take_current_task().unwrap();
    let mut task_inner = task.inner_exclusive_access();
    let task_cx_ptr = &mut task_inner.task_cx as *mut TaskContext;
    task_inner.task_status = TaskStatus::Ready;
    drop(task_inner);
    enqueue_task(task);
    // switch to schedule
    schedule(task_cx_ptr);
}

pub const IDLE_PID: usize = 0;

/// mark zombie / save exit_code / push child in initproc / schedule
pub fn exit_current_and_run_next(exit_code: i32) {
    // take from Processor
    let task = take_current_task().unwrap();

    let pid = task.getpid();
    if pid == IDLE_PID {
        println!(
            "[kernel] Idle process exit with exit_code {} ...",
            exit_code
        );
        if exit_code != 0 {
            //crate::sbi::shutdown(255); //255 == -1 for err hint
            shutdown(true)
        } else {
            //crate::sbi::shutdown(0); //0 for success hint
            shutdown(false)
        }
    }

    // **** access current TCB exclusively
    let mut inner = task.inner_exclusive_access();
    // Change status to Zombie
    inner.task_status = TaskStatus::Zombie;
    // Record exit code
    inner.exit_code = exit_code;
    // do not move to its parent but under initproc

    // ++++++ access initproc TCB exclusively
    // 如果当前任务不是 INITPROC，才需要将子进程移交给 INITPROC
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
        open_file("init", OpenFlags::RDONLY)
            .unwrap()
            .read_all()
            .as_slice()
    ));
}

pub fn add_initproc() {
    enqueue_task(INITPROC.clone());
}
