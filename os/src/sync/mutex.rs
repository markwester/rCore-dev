//! sync: mutex implementation
use super::up::UPSafeCell;
use alloc::collections::VecDeque;
use alloc::sync::Arc;
use crate::task::TaskControlBlock;
use crate::task::{current_task, block_current_and_run_next, suspend_current_and_run_next};

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

impl MutexBlocking {
    pub fn new() -> Self {
        Self {
            inner: unsafe {
                UPSafeCell::new(MutexBlockingInner {
                    locked: false,
                    wait_queue: VecDeque::new(),
                })
            },
        }
    }
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

pub struct MutexSpin {
    inner: UPSafeCell<MutexSpinInner>,
}

pub struct MutexSpinInner {
    locked: bool,
}

impl MutexSpin {
    pub fn new() -> Self {
        Self {
            inner: unsafe {
                UPSafeCell::new(MutexSpinInner {
                    locked: false,
                })
            },
        }
    }
}

impl Mutex for MutexSpin {
    fn lock(&self) {
        loop {
            let mut inner = self.inner.exclusive_access();
            if inner.locked {
                drop(inner);
                suspend_current_and_run_next();
                continue;
            } else {
                inner.locked = true;
                return;
            }
        }
    }
    fn unlock(&self) {
        self.inner.exclusive_access().locked = false;
    }
}