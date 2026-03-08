mod context;
mod switch;
mod task;

use super::loader::get_app_data;
use super::loader::get_num_app;
use super::sync::UPSafeCell;
use crate::sbi::shutdown;
use crate::trap::context::TrapContext;
use alloc::vec::Vec;
use context::TaskContext;
use core::panic;
use lazy_static::lazy_static;
use switch::__switch;
use task::{TaskControlBlock, TaskStatus};

struct TaskManagerInner {
    tasks: Vec<TaskControlBlock>,
    current_task: usize,
}

pub struct TaskManager {
    num_app: usize,
    inner: UPSafeCell<TaskManagerInner>,
}

lazy_static! {
    pub static ref TASK_MANAGER: TaskManager = {
        let num_app = get_num_app();
        let mut tasks: Vec<TaskControlBlock> = Vec::new();

        for i in 0..num_app {
            tasks.push(TaskControlBlock::new(get_app_data(i), i));
        }

        TaskManager {
            num_app,
            inner: unsafe {
                UPSafeCell::new(TaskManagerInner {
                    tasks,
                    current_task: 0,
                })
            },
        }
    };
}

impl TaskManager {
    fn mark_current_suspended(&self) {
        let mut inner = self.inner.exclusive_access();
        let current = inner.current_task;
        inner.tasks[current].task_status = TaskStatus::Ready;
    }

    fn mark_current_exited(&self) {
        let mut inner = self.inner.exclusive_access();
        let current = inner.current_task;
        inner.tasks[current].task_status = TaskStatus::Exited;
    }

    fn run_next_task(&self) {
        if let Some(next) = self.find_next_task() {
            let mut inner = self.inner.exclusive_access();
            inner.tasks[next].task_status = TaskStatus::Running;
            let current = inner.current_task;
            inner.current_task = next;
            let current_task_cx_ptr = &mut inner.tasks[current].task_cx as *mut TaskContext;
            let next_task_cx_ptr = &mut inner.tasks[next].task_cx as *mut TaskContext;
            drop(inner);
            unsafe {
                __switch(current_task_cx_ptr, next_task_cx_ptr);
            }
            // return to user mode
        } else {
            println!("No ready task to run, All applications completed!");
            shutdown(false);
        }
    }

    fn find_next_task(&self) -> Option<usize> {
        let inner = self.inner.exclusive_access();
        let current = inner.current_task;
        (current + 1..current + self.num_app + 1)
            .map(|id| id % self.num_app)
            .find(|id| inner.tasks[*id].task_status == TaskStatus::Ready)
    }

    fn run_first_task(&self) -> ! {
        let mut inner = self.inner.exclusive_access();
        inner.tasks[0].task_status = TaskStatus::Running;
        inner.current_task = 0;
        let first_task_cx_ptr = &mut inner.tasks[0].task_cx as *mut TaskContext;
        drop(inner);
        let mut unused_task_cx = TaskContext::zeroed();
        unsafe {
            __switch(&mut unused_task_cx as *mut TaskContext, first_task_cx_ptr);
        }
        panic!("unreachable in run_first_task!");
    }

    fn get_current_token(&self) -> usize {
        let inner = self.inner.exclusive_access();
        let current = inner.current_task;
        inner.tasks[current].get_user_token()
    }

    fn get_current_trap_cx(&self) -> &mut TrapContext {
        let inner = self.inner.exclusive_access();
        let current = inner.current_task;
        inner.tasks[current].get_trap_cx()
    }
}

pub fn mark_current_suspended() {
    TASK_MANAGER.mark_current_suspended();
}

pub fn mark_current_exited() {
    TASK_MANAGER.mark_current_exited();
}

pub fn run_next_task() {
    TASK_MANAGER.run_next_task();
}

pub fn run_first_task() {
    TASK_MANAGER.run_first_task();
}

pub fn suspend_current_and_run_next() {
    mark_current_suspended();
    run_next_task();
}

pub fn exit_current_and_run_next() {
    mark_current_exited();
    run_next_task();
}

pub fn current_user_token() -> usize {
    TASK_MANAGER.get_current_token()
}

pub fn current_trap_cx() -> &'static mut TrapContext {
    TASK_MANAGER.get_current_trap_cx()
}
