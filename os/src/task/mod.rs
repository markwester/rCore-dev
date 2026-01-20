mod switch;
mod context;
mod task;

use super::sync::UPSafeCell;
use task::TaskControlBlock;
use super::config::MAX_APP_NUM;

struct TaskManagerInner {
    tasks: [TaskControlBlock; MAX_APP_NUM],
    current_task: usize,
}

pub struct TaskManager {
    num_app: usize,
    inner: UPSafeCell<TaskManagerInner>,
}
