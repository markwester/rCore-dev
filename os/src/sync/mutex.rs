//! sync: mutex implementation
use super::up::UPSafeCell;
use alloc::collections::VecDeque;
use alloc::sync::Arc;
use crate::task::TaskControlBlock;
use crate::task::{current_task, block_current_and_run_next};

pub trait Mutex: Sync + Send {
    fn lock(&self);
    fn unlock(&self);
}

pub struct MutexBlocking {
    inner: UPSafeCell<MutexBlockingInner>,
}

pub struct MutexBlockingInner {
    locked: bool,
    wait_queue: VecDeque<Arc<TaskControlBlock>>,
}

impl Mutex for MutexBlocking {
    fn lock(&self) {
        let mut inner = self.inner.exclusive_access();
        if inner.locked {
            inner.wait_queue.push_back(current_task().unwrap());
            drop(inner);
            block_current_and_run_next();
        } else {
            inner.locked = true;
        }
    }
    fn unlock(&self) {
        let mut inner = self.inner.exclusive_access();
        if !inner.locked {
            return
        }
        if let Some(waking_task) = inner.wait_queue.pop_front() {
            inner.locked = false;
            crate::task::wakeup_task(waking_task);
        } else {
            inner.locked = false;
        }
    }
}