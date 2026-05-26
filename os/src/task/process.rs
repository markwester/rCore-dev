//! process management

use super::id::PidHandle;
use super::id::RecycleAllocator;
use super::id::pid_alloc;
use super::manager::pid2pcb_addmap;
use super::{SignalFlags, TaskControlBlock};
use crate::fs::File;
use crate::fs::*;
use crate::mm::MemorySet;
use crate::sync::UPSafeCell;
use crate::task::manager::enqueue_task;
use crate::trap::context::TrapContext;
use crate::trap::trap_handler;
use alloc::sync::{Arc, Weak};
use alloc::vec;
use alloc::vec::Vec;
use core::cell::RefMut;
use alloc::string::String;
use crate::mm::{translated_refmut, kernel_token};
use super::current_task;

pub struct ProcessControlBlock {
    // immutable
    pub pid: PidHandle,
    // mutable
    inner: UPSafeCell<ProcessControlBlockInner>,
}

pub struct ProcessControlBlockInner {
    pub is_zombie: bool,
    pub memory_set: MemorySet,
    pub parent: Option<Weak<ProcessControlBlock>>,
    pub children: Vec<Arc<ProcessControlBlock>>,
    pub exit_code: i32,
    pub fd_table: Vec<Option<Arc<dyn File + Send + Sync>>>,
    pub signals: SignalFlags,
    pub tasks: Vec<Option<Arc<TaskControlBlock>>>,
    pub task_res_allocator: RecycleAllocator,
}

impl ProcessControlBlockInner {
    pub fn alloc_tid(&mut self) -> usize {
        self.task_res_allocator.alloc()
    }

    pub fn dealloc_tid(&mut self, tid: usize) {
        self.task_res_allocator.dealloc(tid)
    }

    pub fn get_task(&self, tid: usize) -> Arc<TaskControlBlock> {
        self.tasks[tid].as_ref().unwrap().clone()
    }
    pub fn alloc_fd(&mut self) -> usize {
        if let Some(fd) = (0..self.fd_table.len()).find(|fd| self.fd_table[*fd].is_none()) {
            fd
        } else {
            self.fd_table.push(None);
            self.fd_table.len() - 1
        }
    }
    pub fn get_user_token(&self) -> usize {
        self.memory_set.token()
    }
}

impl ProcessControlBlock {
    pub fn inner_exclusive_access(&self) -> RefMut<'_, ProcessControlBlockInner> {
        self.inner.exclusive_access()
    }
    pub fn new(elf_data: &[u8]) -> Arc<Self> {
        // memory_set with elf program headers/trampoline/trap context/user stack
        let (memory_set, ustack_base, entry_point) = MemorySet::from_elf(elf_data);
        let pid_handle = pid_alloc();
        let pcb = Arc::new(Self {
            pid: pid_handle,
            inner: unsafe {
                UPSafeCell::new(ProcessControlBlockInner {
                    is_zombie: false,
                    memory_set,
                    parent: None,
                    children: Vec::new(),
                    exit_code: 0,
                    fd_table: vec![
                        // 0 -> stdin
                        Some(Arc::new(Stdin)),
                        // 1 -> stdout
                        Some(Arc::new(Stdout)),
                        // 2 -> stderr
                        Some(Arc::new(Stdout)),
                    ],
                    signals: SignalFlags::empty(),
                    tasks: Vec::new(),
                    task_res_allocator: RecycleAllocator::new(),
                })
            },
        });

        // create main thread
        let tcb = Arc::new(TaskControlBlock::new(Arc::clone(&pcb), ustack_base, true));
        let tcb_inner = tcb.inner_exclusive_access();
        let trap_cx = tcb_inner.get_trap_cx();
        let kernel_sp = tcb.kstack.get_top();
        *trap_cx = TrapContext::app_init_context(
            entry_point,
            tcb_inner.res.as_ref().unwrap().get_ustack_top(),
            kernel_token(),
            kernel_sp,
            trap_handler as usize,
        );
        drop(tcb_inner);

        let mut pcb_inner = pcb.inner_exclusive_access();
        pcb_inner.tasks.push(Some(Arc::clone(&tcb)));
        drop(pcb_inner);
        // insert_into_pid2process(process.getpid(), Arc::clone(&process));
        enqueue_task(tcb);
        pid2pcb_addmap(Arc::clone(&pcb));
        pcb
    }

    /// Only support processes with a single thread.
    pub fn fork(self: &Arc<Self>) -> Arc<Self> {
        // ---- access parent PCB exclusively
        let mut parent_inner = self.inner_exclusive_access();
        assert_eq!(parent_inner.tasks.len(), 1);
        // copy user space(include trap context)
        let memory_set = MemorySet::from_existed_user(&parent_inner.memory_set);
        // alloc a pid and a kernel stack in kernel space
        let pid_handle = pid_alloc();

        let mut new_fd_table: Vec<Option<Arc<dyn File + Send + Sync>>> = Vec::new();
        for fd in parent_inner.fd_table.iter() {
            if let Some(file) = fd {
                new_fd_table.push(Some(file.clone()));
            } else {
                new_fd_table.push(None);
            }
        }
        let pcb = Arc::new(Self {
            pid: pid_handle,
            inner: unsafe {
                UPSafeCell::new(ProcessControlBlockInner {
                    is_zombie: false,
                    memory_set,
                    parent: None,
                    children: Vec::new(),
                    exit_code: 0,
                    fd_table: new_fd_table,
                    signals: SignalFlags::empty(),
                    tasks: Vec::new(),
                    task_res_allocator: RecycleAllocator::new(),
                })
            },
        });
        parent_inner.children.push(Arc::clone(&pcb));
        pid2pcb_addmap(Arc::clone(&pcb));

        // create main thread
        let ustack_base = parent_inner
            .get_task(0)
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .get_ustack_base();
        let tcb = Arc::new(TaskControlBlock::new(Arc::clone(&pcb), ustack_base, false));
        pcb.inner_exclusive_access().tasks.push(Some(Arc::clone(&tcb)));

        // set kstack sp in trap_cx, other are inherited from parent process
        let tcb_inner = tcb.inner_exclusive_access();
        let trap_cx = tcb_inner.get_trap_cx();
        let kernel_sp = tcb.kstack.get_top();
        trap_cx.kernel_sp = kernel_sp;
        drop(tcb_inner);
        enqueue_task(tcb);
        pcb
    }

    /// Only support processes with a single thread.
    /// placed modify self process's somethings
    pub fn exec(self: &Arc<Self>, elf_data: &[u8], args: Vec<String>) {
        assert!(self.inner_exclusive_access().tasks.len() == 1);

        let (memory_set, ustack_base, entry_point) = MemorySet::from_elf(elf_data);
        let new_token = memory_set.token();
        self.inner_exclusive_access().memory_set = memory_set;

        // alloc new trap
        let task = self.inner_exclusive_access().get_task(0);
        let mut task_inner = task.inner_exclusive_access();
        task_inner.res.as_mut().unwrap().ustack_base = ustack_base;
        task_inner.res.as_mut().unwrap().alloc_user_res();
        task_inner.trap_cx_ppn = task_inner.res.as_mut().unwrap().trap_cx_ppn();


        // push arguments on user stack
        let mut user_sp = task_inner.res.as_mut().unwrap().get_ustack_top();
        user_sp -= (args.len() + 1) * core::mem::size_of::<usize>();
        let argv_base = user_sp;
        // 获取高位存储字符串地址的用户空间地址
        let mut argv: Vec<_> = (0..=args.len())
            .map(|arg| {
                translated_refmut(
                    new_token,
                    (argv_base + arg * core::mem::size_of::<usize>()) as *mut usize,
                )
            })
            .collect();
        // 从底向上写入字符串
        // 最后一个为0 有啥作用？标记参数结束吧
        // 为什么各数据排列是高地址->低地址，但每个数据内排列时低->高
        *argv[args.len()] = 0;
        for i in 0..args.len() {
            // 计算该参数的起始地址，加1预留一个'\0'
            user_sp -= args[i].len() + 1;
            *argv[i] = user_sp;
            let mut p: usize = user_sp;
            // 按char逐一copy
            for c in args[i].as_bytes() {
                *translated_refmut(new_token, p as *mut u8) = *c;
                p += 1;
            }
            // 最后写入'\0'
            *translated_refmut(new_token, p as *mut u8) = 0;
        }

        // make the user_sp aligned to 8B for k210 platform
        user_sp -= user_sp % core::mem::size_of::<usize>();
        // initialize trap_cx
        let trap_cx: &mut TrapContext = task.inner_exclusive_access().get_trap_cx();
        *trap_cx = TrapContext::app_init_context(
            entry_point,
            user_sp,
            kernel_token(),
            task.kstack.get_top(),
            trap_handler as usize,
        );
        // 必须在 app_init_context 之后设置 argc/argv，否则会被覆盖
        trap_cx.x[10] = args.len();
        trap_cx.x[11] = argv_base;
        // **** stop exclusively accessing inner automatically
    }

    pub fn getpid(&self) -> usize {
        self.pid.0
    }
}

pub fn current_process() -> Arc<ProcessControlBlock> {
    current_task().unwrap().process.upgrade().unwrap()
}