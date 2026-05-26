pub mod action;
mod context;
pub mod manager;
// mod pid;
mod id;
mod process;
pub mod processor;
mod signal;
mod switch;
mod task;

use crate::fs::{OpenFlags, open_file};
use crate::sbi::shutdown;
use crate::timer::remove_timer;
use alloc::sync::Arc;
use alloc::vec::Vec;
use context::TaskContext;
use id::TaskUserRes;
use lazy_static::lazy_static;
pub use manager::enqueue_task;
use manager::pid2pcb_delmap;
use manager::remove_task;
pub use manager::{wakeup_task, pid2pcb};
use process::ProcessControlBlock;
pub use processor::{current_task, current_trap_cx, current_user_token, run_tasks};
use processor::{schedule, take_current_task};
pub use signal::{SignalFlags};
use switch::__switch;
pub use task::TaskControlBlock;
use task::TaskStatus;
pub use process::current_process;

pub fn suspend_current_and_run_next() {
    // mark current task as suspended and enqueue it
    let task = take_current_task().unwrap();
    let mut task_inner = task.inner_exclusive_access();
    let task_cx_ptr = &mut task_inner.task_cx as *mut TaskContext;
    task_inner.task_status = TaskStatus::Ready;
    drop(task_inner);
    enqueue_task(task);
    // switch to schedule
    schedule(task_cx_ptr);
}

pub const IDLE_PID: usize = 0;

/// mark zombie / save exit_code / push child in initproc / schedule
pub fn exit_current_and_run_next(exit_code: i32) {
    // take from Processor
    let task = take_current_task().unwrap();
    let process = task.process.upgrade().unwrap();
    let mut task_inner = task.inner_exclusive_access();
    let tid = task_inner.res.as_ref().unwrap().tid;

    task_inner.exit_code = Some(exit_code);
    task_inner.res = None;
    drop(task_inner);

    // main thread exit, kill all threads and this process
    if tid == 0 {
        let pid = process.getpid();
        // if idle process exit, shutdown
        if pid == IDLE_PID {
            println!(
                "[kernel] Idle process exit with exit_code {} ...",
                exit_code
            );
            if exit_code != 0 {
                //crate::sbi::shutdown(255); //255 == -1 for err hint
                shutdown(true)
            } else {
                //crate::sbi::shutdown(0); //0 for success hint
                shutdown(false)
            }
        }

        pid2pcb_delmap(pid);
        let mut process_inner = process.inner_exclusive_access();
        process_inner.is_zombie = true;
        process_inner.exit_code = exit_code;

        // move all children to initproc
        {
            let mut initproc_inner = INITPROC.inner_exclusive_access();
            for child in process_inner.children.iter() {
                child.inner_exclusive_access().parent = Some(Arc::downgrade(&INITPROC));
                initproc_inner.children.push(child.clone());
            }
        }

        // deallocate user res (including tid/trap_cx/ustack) of all threads
        let mut recycle_res = Vec::<TaskUserRes>::new();
        for task in process_inner.tasks.iter().filter(|t| t.is_some()) {
            let task = task.as_ref().unwrap();
            // if other tasks are Ready in TaskManager or waiting for a timer to be
            // expired, we should remove them.
            //
            // Mention that we do not need to consider Mutex/Semaphore since they
            // are limited in a single process. Therefore, the blocked tasks are
            // removed when the PCB is deallocated.
            remove_inactive_task(Arc::clone(&task));
            let mut task_inner = task.inner_exclusive_access();
            if let Some(res) = task_inner.res.take() {
                {
                    recycle_res.push(res);
                }
            }
        }
        // dealloc_tid and dealloc_user_res require access to PCB inner, so we
        // need to collect those user res first, then release process_inner
        // for now to avoid deadlock/double borrow problem.
        drop(process_inner);
        recycle_res.clear();

        let mut process_inner = process.inner_exclusive_access();
        process_inner.children.clear();
        process_inner.memory_set.recycle_data_pages();
        process_inner.fd_table.clear();
        while process_inner.tasks.len() > 1 {
            process_inner.tasks.pop();
        }
    }
    drop(process);
    let mut _unused = TaskContext::zeroed();
    schedule(&mut _unused as *mut _);
}

lazy_static! {
    pub static ref INITPROC: Arc<ProcessControlBlock> = {
        let inode = open_file("init", OpenFlags::RDONLY).unwrap();
        let v = inode.read_all();
        ProcessControlBlock::new(v.as_slice())
    };
}

pub fn init_initproc() {
    let _initproc = INITPROC.clone();
}

pub fn current_add_signal(signal: SignalFlags) {
    let pcb = current_process();
    let mut pcb_inner = pcb.inner_exclusive_access();
    pcb_inner.signals |= signal;
}

// fn call_kernel_signal_handler(signal: SignalFlags) {
//     let task = current_task().unwrap();
//     let mut task_inner = task.inner_exclusive_access();
//     task_inner.signals.remove(signal);
//     if signal == SignalFlags::SIGSTOP {
//         task_inner.frozen = true;
//     } else if signal == SignalFlags::SIGCONT {
//         task_inner.frozen = false;
//     }
//     task_inner.killed = true;
// }

// fn call_user_signal_handler(sig: usize, signal: SignalFlags) {
//     let task = current_task().unwrap();
//     let mut task_inner = task.inner_exclusive_access();

//     task_inner.handling_sig = sig as isize;

//     let handler = task_inner.signal_actions.table[sig].handler;
//     if handler != 0 {
//         task_inner.signals.remove(signal);
//         let trap_cx = task_inner.get_trap_cx();
//         task_inner.trap_ctx_backup = Some(*trap_cx);
//         trap_cx.sepc = task_inner.signal_actions.table[sig].handler as usize;
//         trap_cx.x[10] = sig; // a0 = signal number
//     } else {
//         println!("[K] task/call_user_signal_handler: default action: ignore it or kill process");
//     }
// }

// fn check_pending_signals() {
//     for sig in 0..(MAX_SIG + 1) {
//         let task = current_task().unwrap();
//         let task_inner = task.inner_exclusive_access();
//         let sig_flag = SignalFlags::from_bits(1 << sig).unwrap();
//         let signal = SignalFlags::from_bits(1 << sig).unwrap();
//         if task_inner.signals.contains(sig_flag) && !task_inner.signal_mask.contains(sig_flag) {
//             // 避免同步信号处理过程中trap到内核被屏蔽信号嵌套处理
//             // 比如先走了call_user_signal_handler，回到用户态执行信号处理函数期间，trap进来回用户态前又进入这里处理新的已被屏蔽的信号
//             let mut masked = true;
//             let handling_sig = task_inner.handling_sig;
//             if handling_sig == -1 {
//                 masked = false;
//             } else {
//                 if !task_inner.signal_actions.table[handling_sig as usize]
//                     .mask
//                     .contains(sig_flag)
//                 {
//                     masked = false;
//                 }
//             }

//             if !masked {
//                 drop(task_inner);
//                 drop(task);
//                 if signal == SignalFlags::SIGKILL
//                     || signal == SignalFlags::SIGSTOP
//                     || signal == SignalFlags::SIGCONT
//                     || signal == SignalFlags::SIGDEF
//                 {
//                     // signal is a kernel signal
//                     call_kernel_signal_handler(signal);
//                 } else {
//                     // signal is a user signal
//                     call_user_signal_handler(sig, signal);
//                     return;
//                 }
//             }
//         }
//     }
// }

// pub fn handle_signals() {
//     loop {
//         check_pending_signals();
//         let (frozen, killed) = {
//             let task = current_task().unwrap();
//             let task_inner = task.inner_exclusive_access();
//             (task_inner.frozen, task_inner.killed)
//         };
//         if !frozen || killed {
//             break;
//         }
//         suspend_current_and_run_next();
//     }
// }

pub fn check_signals_error_of_current() -> Option<(i32, &'static str)> {
    let pcb = current_process();
    let pcb_inner = pcb.inner_exclusive_access();
    pcb_inner.signals.check_error()
}

pub fn remove_inactive_task(task: Arc<TaskControlBlock>) {
    remove_task(Arc::clone(&task));
    remove_timer(Arc::clone(&task));
}
