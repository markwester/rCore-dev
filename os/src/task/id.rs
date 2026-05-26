//! usual id allocator for process and thread
use super::process::ProcessControlBlock;
use crate::config::*;
use crate::mm::KERNEL_SPACE;
use crate::mm::PhysPageNum;
use crate::mm::VirtAddr;
use crate::{mm::MapPermission, sync::UPSafeCell};
use alloc::sync::{Arc, Weak};
use alloc::vec::Vec;
use lazy_static::lazy_static;

pub struct RecycleAllocator {
    current: usize,
    recycled: Vec<usize>,
}

impl RecycleAllocator {
    pub fn new() -> Self {
        RecycleAllocator {
            current: 0,
            recycled: Vec::new(),
        }
    }
    pub fn alloc(&mut self) -> usize {
        if let Some(id) = self.recycled.pop() {
            id
        } else {
            self.current += 1;
            self.current - 1
        }
    }
    pub fn dealloc(&mut self, id: usize) {
        assert!(id < self.current);
        assert!(
            !self.recycled.iter().any(|i| *i == id),
            "id {} has been deallocated!",
            id
        );
        self.recycled.push(id);
    }
}

pub struct PidHandle(pub usize);

lazy_static! {
    static ref PID_ALLOCATOR: UPSafeCell<RecycleAllocator> =
        unsafe { UPSafeCell::new(RecycleAllocator::new()) };
}

pub fn pid_alloc() -> PidHandle {
    PidHandle(PID_ALLOCATOR.exclusive_access().alloc())
}

impl Drop for PidHandle {
    fn drop(&mut self) {
        PID_ALLOCATOR.exclusive_access().dealloc(self.0);
    }
}

fn trap_cx_bottom_from_tid(tid: usize) -> usize {
    TRAP_CONTEXT_BASE - tid * PAGE_SIZE
}

fn ustack_bottom_from_tid(ustack_base: usize, tid: usize) -> usize {
    ustack_base + tid * (PAGE_SIZE + USER_STACK_SIZE)
}

pub struct TaskUserRes {
    pub tid: usize,
    pub ustack_base: usize,
    pub process: Weak<ProcessControlBlock>,
}

impl TaskUserRes {
    pub fn new(
        process: Arc<ProcessControlBlock>,
        ustack_base: usize,
        alloc_user_res: bool,
    ) -> Self {
        let tid = process.inner_exclusive_access().alloc_tid();
        let task_user_res = Self {
            tid,
            ustack_base,
            process: Arc::downgrade(&process),
        };
        if alloc_user_res {
            task_user_res.alloc_user_res();
        }
        task_user_res
    }

    /// 在进程地址空间中实际映射线程的用户栈和 Trap 上下文
    pub fn alloc_user_res(&self) {
        let process = self.process.upgrade().unwrap();
        let mut process_inner = process.inner_exclusive_access();

        let ustack_bottom_va = ustack_bottom_from_tid(self.ustack_base, self.tid);
        let ustack_top_va = ustack_bottom_va + USER_STACK_SIZE;
        process_inner.memory_set.insert_framed_area(
            ustack_bottom_va.into(),
            ustack_top_va.into(),
            MapPermission::R | MapPermission::W | MapPermission::U,
        );

        let trap_ctx_bottom_va = trap_cx_bottom_from_tid(self.tid);
        let trap_ctx_top_va = trap_ctx_bottom_va + PAGE_SIZE;
        process_inner.memory_set.insert_framed_area(
            trap_ctx_bottom_va.into(),
            trap_ctx_top_va.into(),
            MapPermission::R | MapPermission::W,
        );
    }

    fn dealloc_user_res(&self) {
        self.dealloc_tid();
        let process = self.process.upgrade().unwrap();
        let mut process_inner = process.inner_exclusive_access();

        let ustack_bottom_va = ustack_bottom_from_tid(self.ustack_base, self.tid);
        process_inner
            .memory_set
            .remove_area_with_start_vpn(ustack_bottom_va.into());

        let trap_ctx_bottom_va = trap_cx_bottom_from_tid(self.tid);
        process_inner
            .memory_set
            .remove_area_with_start_vpn(trap_ctx_bottom_va.into());
    }

    pub fn dealloc_tid(&self) {
        let process = self.process.upgrade().unwrap();
        let mut process_inner = process.inner_exclusive_access();
        process_inner.dealloc_tid(self.tid);
    }

    pub fn trap_cx_ppn(&self) -> PhysPageNum {
        // trap_cx_bottom_from_tid(self.tid).int
        let process = self.process.upgrade().unwrap();
        let process_inner = process.inner_exclusive_access();
        process_inner
            .memory_set
            .translate(trap_cx_bottom_from_tid(self.tid).into())
            .unwrap()
            .ppn()
    }

    pub fn get_ustack_top(&self) -> usize {
        ustack_bottom_from_tid(self.ustack_base, self.tid) + USER_STACK_SIZE
    }

    pub fn get_ustack_base(&self) -> usize {
        self.ustack_base
    }
}

impl Drop for TaskUserRes {
    fn drop(&mut self) {
        self.dealloc_user_res();
    }
}

lazy_static! {
    static ref KSTACK_ALLOCATOR: UPSafeCell<RecycleAllocator> =
        unsafe { UPSafeCell::new(RecycleAllocator::new()) };
}

pub struct KernelStack(pub usize);

/// Return (bottom, top) of a kernel stack in kernel space.
pub fn kernel_stack_position(kstack_id: usize) -> (usize, usize) {
    let top = TRAMPOLINE - kstack_id * (KERNEL_STACK_SIZE + PAGE_SIZE);
    let bottom = top - KERNEL_STACK_SIZE;
    (bottom, top)
}

pub fn kstack_alloc() -> KernelStack {
    let kstack_id = KSTACK_ALLOCATOR.exclusive_access().alloc();
    let (kstack_bottom, kstack_top) = kernel_stack_position(kstack_id);
    KERNEL_SPACE.exclusive_access().insert_framed_area(
        kstack_bottom.into(),
        kstack_top.into(),
        MapPermission::R | MapPermission::W,
    );
    KernelStack(kstack_id)
}

impl KernelStack {
    pub fn get_top(&self) -> usize {
        let (_, top) = kernel_stack_position(self.0);
        top
    }
}

impl Drop for KernelStack {
    fn drop(&mut self) {
        let (kernel_stack_bottom, _) = kernel_stack_position(self.0);
        let kernel_stack_bottom_va: VirtAddr = kernel_stack_bottom.into();
        KERNEL_SPACE
            .exclusive_access()
            .remove_area_with_start_vpn(kernel_stack_bottom_va.into());
        KSTACK_ALLOCATOR.exclusive_access().dealloc(self.0);
    }
}
