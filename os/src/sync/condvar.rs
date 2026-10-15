use alloc::collections::VecDeque;
use alloc::sync::Arc;
use super::up::UPSafeCell;
use crate::task::{TaskControlBlock, current_task, block_current_and_run_next, wakeup_task};
use super::mutex::Mutex;

pub struct Condvar {
    pub inner: UPSafeCell<CondvarInner>,
}

pub struct CondvarInner {
    pub wait_queue: VecDeque<Arc<TaskControlBlock>>,
}

impl Condvar {
    pub fn new() -> Self {
        Self {
            inner: unsafe {
                UPSafeCell::new(CondvarInner {
                    wait_queue: VecDeque::new(),
            })},
        }
    }
    pub fn wait(&self, mutex: Arc<dyn Mutex>) {
        mutex.unlock();
        let mut inner = self.inner.exclusive_access();
        inner.wait_queue.push_back(current_task().unwrap());
        drop(inner);
        block_current_and_run_next();
        mutex.lock();
    }

    pub fn signals(&self) {
        let mut inner = self.inner.exclusive_access();
        if let Some(waking_task) = inner.wait_queue.pop_front() {
            wakeup_task(waking_task);
        }
    }
}