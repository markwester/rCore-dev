//! task manager

use super::task::TaskControlBlock;
use alloc::collections::VecDeque;
use alloc::sync::Arc;
use lazy_static::lazy_static;
use crate::sync::UPSafeCell;
use alloc::collections::BTreeMap;
use super::process::ProcessControlBlock;
use super::task::TaskStatus;

pub struct TaskManager {
    ready_queue: VecDeque<Arc<TaskControlBlock>>,
}

/// A simple FIFO scheduler.
impl TaskManager {
    pub fn new() -> Self {
        Self { ready_queue: VecDeque::new(), }
    }
    pub fn enqueue(&mut self, task: Arc<TaskControlBlock>) {
        self.ready_queue.push_back(task);
    }
    pub fn dequeue(&mut self) -> Option<Arc<TaskControlBlock>> {
        self.ready_queue.pop_front()
    }
    pub fn remove(&mut self, task: Arc<TaskControlBlock>) {
        if let Some((id, _)) = self
            .ready_queue
            .iter()
            .enumerate()
            .find(|(_, t)| Arc::as_ptr(t) == Arc::as_ptr(&task))
        {
            self.ready_queue.remove(id);
        }
    }

}

lazy_static! {
    pub static ref TASK_MANAGER: UPSafeCell<TaskManager> = unsafe {
        UPSafeCell::new(TaskManager::new())
    };
    pub static ref PID2PCB_MAP: UPSafeCell<BTreeMap<usize, Arc<ProcessControlBlock>>> = unsafe {
        UPSafeCell::new(BTreeMap::new())
    };
}

pub fn enqueue_task(task: Arc<TaskControlBlock>) {
    TASK_MANAGER.exclusive_access().enqueue(task);
}

pub fn dequeue_task() -> Option<Arc<TaskControlBlock>> {
    TASK_MANAGER.exclusive_access().dequeue()
}

pub fn remove_task(task: Arc<TaskControlBlock>) {
    TASK_MANAGER.exclusive_access().remove(task);
}

pub fn pid2pcb_addmap(pcb: Arc<ProcessControlBlock>) {
    PID2PCB_MAP.exclusive_access().insert(pcb.getpid(), pcb);
}

pub fn pid2pcb_delmap(pid: usize) {
    if PID2PCB_MAP.exclusive_access().remove(&pid).is_none() {
        panic!("can not find pid {} in pid2task_map", pid);
    }
}

pub fn pid2pcb(pid: usize) -> Option<Arc<ProcessControlBlock>> {
    PID2PCB_MAP.exclusive_access().get(&pid).map(|task| task.clone())
}

pub fn wakeup_task(task: Arc<TaskControlBlock>) {
    let mut task_inner = task.inner_exclusive_access();
    task_inner.task_status = TaskStatus::Ready;
    drop(task_inner);
    enqueue_task(task);
}