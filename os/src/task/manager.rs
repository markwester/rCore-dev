//! task manager

use super::task::TaskControlBlock;
use alloc::collections::VecDeque;
use alloc::sync::Arc;
use lazy_static::lazy_static;
use crate::sync::UPSafeCell;
use super::processor::take_current_task;
use super::TaskStatus;

pub struct TaskManager {
    ready_queue: VecDeque<Arc<TaskControlBlock>>,
}

/// A simple FIFO scheduler.
impl TaskManager {
    pub fn new() -> Self {
        Self { ready_queue: VecDeque::new(), }
    }
    pub fn enqueue_task(&mut self, task: Arc<TaskControlBlock>) {
        self.ready_queue.push_back(task);
    }
    pub fn dequeue_task(&mut self) -> Option<Arc<TaskControlBlock>> {
        self.ready_queue.pop_front()
    }

    pub fn mark_current_suspended(&mut self) {
        if let Some(task) = take_current_task() {
            task.inner_exclusive_access().task_status = TaskStatus::Ready;
            self.enqueue_task(task);
        }
    }
}

lazy_static! {
    pub static ref TASK_MANAGER: UPSafeCell<TaskManager> = unsafe {
        UPSafeCell::new(TaskManager::new())
    };
}

pub fn enqueue_task(task: Arc<TaskControlBlock>) {
    TASK_MANAGER.exclusive_access().enqueue_task(task);
}

pub fn dequeue_task() -> Option<Arc<TaskControlBlock>> {
    TASK_MANAGER.exclusive_access().dequeue_task()
}