use crate::fs::{OpenFlags, open_file, make_pipe};
use crate::mm::page_table::translated_byte_buffer;
use crate::mm::{UserBuffer, translated_str, translated_refmut};
use crate::task::current_task;
use crate::task::current_user_token;
use alloc::sync::Arc;

/// the common function for syscall write and read
fn rw_file(fd: usize, buf: *const u8, len: usize, is_write: bool) -> isize {
    let task = current_task().unwrap();
    let inner = task.inner_exclusive_access();
    let fd_table = &inner.fd_table;
    if fd >= fd_table.len() || fd_table[fd].is_none() {
        return -1;
    }
    if let Some(file) = &fd_table[fd] {
        let file = file.clone();
        drop(inner);
        let buffers = translated_byte_buffer(current_user_token(), buf, len);
        if is_write {
            file.write(UserBuffer::new(buffers)) as isize
        } else {
            file.read(UserBuffer::new(buffers)) as isize
        }
    } else {
        -1
    }
}

/// write buf of length `len`  to a file with `fd`
pub fn sys_write(fd: usize, buf: *const u8, len: usize) -> isize {
    rw_file(fd, buf, len, true)
}

/// read buf of length `len` from a file with `fd`
pub fn sys_read(fd: usize, buf: *const u8, len: usize) -> isize {
    rw_file(fd, buf, len, false)
}

/// open a file with `name` and `flags`, return the fd of this file
pub fn sys_open(path: *const u8, flags: u32) -> isize {
    // create this vfs inode and add fd to current process's fd table
    let task = current_task().unwrap();
    let token = current_user_token();
    let path = translated_str(token, path);
    if let Some(file) = open_file(path.as_str(), OpenFlags::from_bits(flags).unwrap()) {
        let mut task_inner = task.inner_exclusive_access();
        let fd = task_inner.alloc_fd();
        task_inner.fd_table[fd] = Some(file);
        fd as isize
    } else {
        -1
    }
}

/// close the file with `fd`
pub fn sys_close(fd: usize) -> isize {
    // delete this vfs inode and remove fd from current process's fd table
    let task = current_task().unwrap();
    let mut task_inner = task.inner_exclusive_access();
    if fd > task_inner.fd_table.len() {
        return -1;
    }
    if task_inner.fd_table[fd].is_none() {
        return -1;
    }
    task_inner.fd_table[fd] = None;
    0
}

/// get pipe (pipe_read, pipe_write)
pub fn sys_pipe(pipe: *mut usize) -> isize {
    let task = current_task().unwrap();
    let token = current_user_token();
    let mut inner = task.inner_exclusive_access();
    let (pipe_read, pipe_write) = make_pipe();
    let read_fd = inner.alloc_fd();
    inner.fd_table[read_fd] = Some(pipe_read);
    let write_fd = inner.alloc_fd();
    inner.fd_table[write_fd] = Some(pipe_write);
    *translated_refmut(token, pipe) = read_fd;
    *translated_refmut(token, unsafe { pipe.add(1) }) = write_fd;
    0
}

pub fn sys_dup(fd: usize) -> isize {
    let task = current_task().unwrap();
    let mut task_inner = task.inner_exclusive_access();
    if fd >= task_inner.fd_table.len() {
        return -1;
    }
    if task_inner.fd_table[fd].is_none() {
        return -1;
    }

    let new_fd = task_inner.alloc_fd();
    task_inner.fd_table[new_fd] = Some(Arc::clone(task_inner.fd_table[fd].as_ref().unwrap()));
    new_fd as isize
}