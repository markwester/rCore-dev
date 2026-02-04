#[derive(Copy, Clone)]

pub struct TaskContext {
    ra: usize,
    sp: usize,
    s: [usize; 12],
}

impl TaskContext {
    pub fn zeroed() -> Self {
        TaskContext {
            ra: 0,
            sp: 0,
            s: [0; 12],
        }
    }

    pub fn goto_restore(kstack_sp: usize) -> Self {
        unsafe extern "C" {
            fn __restore(cx_addr: usize);
        }
        Self {
            ra: __restore as usize,
            sp: kstack_sp,
            s: [0; 12],
        }
    }
}