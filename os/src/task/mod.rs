mod context;
mod switch;
mod task;

use core::panic;

use super::config::MAX_APP_NUM;
use super::loader::get_num_app;
use super::sync::UPSafeCell;
use crate::batch::init_ctx_and_push_kstack;
use context::TaskContext;
use task::{TaskControlBlock, TaskStatus};

use lazy_static::lazy_static;

struct TaskManagerInner {
    tasks: [TaskControlBlock; MAX_APP_NUM],
    current_task: usize,
}

pub struct TaskManager {
    num_app: usize,
    inner: UPSafeCell<TaskManagerInner>,
}

lazy_static! {
    pub static ref TASK_MANAGER: TaskManager = {
        let num_app = get_num_app();
        let mut tasks = [TaskControlBlock {
            task_status: TaskStatus::UnInit,
            task_cx: TaskContext::zeroed(),
        }; MAX_APP_NUM];

        for i in 0..num_app {
            tasks[i].task_status = TaskStatus::Ready;
            tasks[i].task_cx = TaskContext::goto_restore(init_ctx_and_push_kstack(i));
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
                switch::__switch(current_task_cx_ptr, next_task_cx_ptr);
            }
            // return to user mode
        } else {
            panic!("No ready task to run, All applications completed!");
        }
    }

    fn find_next_task(&self) -> Option<usize> {
        let inner = self.inner.exclusive_access();
        let mut next = (inner.current_task + 1) % self.num_app;
        for _ in 0..self.num_app {
            if inner.tasks[next].task_status == TaskStatus::Ready {
                return Some(next);
            }
            next = (next + 1) % self.num_app;
        }
        None
    }

    fn run_first_task(&self) -> ! {
        let mut inner = self.inner.exclusive_access();
        inner.tasks[0].task_status = TaskStatus::Running;
        inner.current_task = 0;
        let first_task_cx_ptr = &mut inner.tasks[0].task_cx as *mut TaskContext;
        drop(inner);
        let mut unused_task_cx = TaskContext::zeroed();
        unsafe {
            switch::__switch(&mut unused_task_cx as *mut TaskContext, first_task_cx_ptr);
        }
        panic!("unreachable in run_first_task!");
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
