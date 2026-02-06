use crate::sbi::set_timer;
use riscv::register::time;
use crate::config::CLOCK_FREQ;

const TICK_PER_SEC: usize = 100;
const MSEC_PER_SEC: usize = 1000;

pub fn get_time() -> usize {
    time::read()
}

pub fn get_time_us() -> usize {
    time::read() / (CLOCK_FREQ / MSEC_PER_SEC)
}

pub fn set_next_tick() {
    set_timer(get_time() + CLOCK_FREQ / TICK_PER_SEC);
}