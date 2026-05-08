//! task manager

use super::task::TaskControlBlock;
use alloc::collections::VecDeque;
use alloc::sync::Arc;
use lazy_static::lazy_static;
use crate::sync::UPSafeCell;
use alloc::collections::BTreeMap;

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

}

lazy_static! {
    pub static ref TASK_MANAGER: UPSafeCell<TaskManager> = unsafe {
        UPSafeCell::new(TaskManager::new())
    };
    pub static ref PID2TASK_MAP: UPSafeCell<BTreeMap<usize, Arc<TaskControlBlock>>> = unsafe {
        UPSafeCell::new(BTreeMap::new())
    };
}

pub fn enqueue_task(task: Arc<TaskControlBlock>) {
    pid2task_addmap(task.clone());
    TASK_MANAGER.exclusive_access().enqueue_task(task);
}

pub fn dequeue_task() -> Option<Arc<TaskControlBlock>> {
    TASK_MANAGER.exclusive_access().dequeue_task()
}

pub fn pid2task_addmap(task: Arc<TaskControlBlock>) {
    PID2TASK_MAP.exclusive_access().insert(task.getpid(), task);
}

pub fn pid2task_delmap(pid: usize) {
    if PID2TASK_MAP.exclusive_access().remove(&pid).is_none() {
        panic!("can not find pid {} in pid2task_map", pid);
    }
}

pub fn pid2task(pid: usize) -> Option<Arc<TaskControlBlock>> {
    PID2TASK_MAP.exclusive_access().get(&pid).map(|task| task.clone())
}