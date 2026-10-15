//! sync: semaphore implementation

use super::up::UPSafeCell;
use crate::task::TaskControlBlock;
use alloc::collections::VecDeque;
use alloc::sync::Arc;
use crate::task::{current_task, block_current_and_run_next, wakeup_task};

pub struct Semaphore {
    inner: UPSafeCell<SemaphoreInner>,
}

pub struct SemaphoreInner {
    count: isize,
    wait_queue: VecDeque<Arc<TaskControlBlock>>,
}

impl Semaphore {
    pub fn new(count: usize) -> Self {
        Self {
            inner: unsafe {
                UPSafeCell::new(SemaphoreInner {
                    count: count as isize,
                    wait_queue: VecDeque::new(),
                })
            },
        }
    }

    pub fn up(&self) {
        let mut inner = self.inner.exclusive_access();
        inner.count += 1;
        if inner.count > 0 {
            if let Some(task) = inner.wait_queue.pop_front() {
                wakeup_task(task);
            }
        }
    }

    pub fn down(&self) {
        let mut inner = self.inner.exclusive_access();
        if inner.count > 0 {
            inner.count -= 1;
        } else {
            inner.wait_queue.push_back(current_task().unwrap());
            drop(inner);
            block_current_and_run_next();
        }
    }
}
