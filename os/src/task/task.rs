//! Task Control Block (TCB) implementation

use super::context::TaskContext;
use super::id::{KernelStack, TaskUserRes};
use crate::mm::address::{PhysPageNum};
use crate::sync::UPSafeCell;
use crate::trap::context::TrapContext;
use alloc::sync::{Arc, Weak};
use core::cell::RefMut;
use super::process::ProcessControlBlock;
use super::id::kstack_alloc;

#[allow(unused)]
#[derive(Copy, Clone, PartialEq)]
pub enum TaskStatus {
    // UnInit, // 未初始化
    Ready,   // 准备运行
    Running, // 正在运行
    // Exited,  // 已退出
    Zombie, // 僵尸状态，已退出但父进程尚未回收
}

pub struct TaskControlBlockInner {
    pub res: Option<TaskUserRes>,
    pub trap_cx_ppn: PhysPageNum,
    pub task_cx: TaskContext,
    pub task_status: TaskStatus,
    pub exit_code: Option<i32>,
}

pub struct TaskControlBlock {
    pub process: Weak<ProcessControlBlock>,
    pub kstack: KernelStack,
    pub inner: UPSafeCell<TaskControlBlockInner>,
}

impl TaskControlBlockInner {
    pub fn get_trap_cx(&self) -> &'static mut TrapContext {
        self.trap_cx_ppn.get_mut()
    }
    #[allow(unused)]
    fn get_status(&self) -> TaskStatus {
        self.task_status
    }
    #[allow(unused)]
    pub fn is_zombie(&self) -> bool {
        self.get_status() == TaskStatus::Zombie
    }
}

impl TaskControlBlock {
    pub fn new(pcb: Arc<ProcessControlBlock>, ustack_base: usize, alloc_user_res: bool) -> Self {
        let user_res = TaskUserRes::new(pcb.clone(), ustack_base, alloc_user_res);
        let kstack = kstack_alloc();
        let kstack_top = kstack.get_top();
        let tcb: TaskControlBlock = Self {
            process: Arc::downgrade(&pcb),
            kstack: kstack,
            inner: unsafe {
                UPSafeCell::new(TaskControlBlockInner {
                    res: None,
                    trap_cx_ppn: user_res.trap_cx_ppn(),
                    task_cx: TaskContext::goto_trap_return(kstack_top),
                    task_status: TaskStatus::Ready,
                    exit_code: None,
                })
            },
        };
        tcb
    }

    #[allow(unused)]
    pub fn gettid(&self) -> usize {
        self.inner_exclusive_access().res.as_ref().unwrap().tid
    }

    pub fn inner_exclusive_access(&self) -> RefMut<'_, TaskControlBlockInner> {
        self.inner.exclusive_access()
    }
    #[allow(unused)]
    pub fn get_user_token(&self) -> usize {
        let process = self.process.upgrade().unwrap();
        let inner = process.inner_exclusive_access();
        inner.memory_set.token()
    }
}
