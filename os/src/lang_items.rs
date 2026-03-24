use crate::sbi::shutdown;
use core::panic::PanicInfo;
use crate::unwind::print_callstack;

#[panic_handler]
pub fn panic(info: &PanicInfo) -> ! {
    if let Some(location) = info.location() {
        println!(
            "[kernel] Panicked at {}:{} {}",
            location.file(),
            location.line(),
            info.message()
        );
    } else {
        println!("[kernel] Panicked: {}", info.message());
    }
    print_callstack();
    shutdown(true)
}
