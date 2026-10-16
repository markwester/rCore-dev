use core::panic::PanicInfo;
use crate::exit;

#[panic_handler]
pub fn panic(info: &PanicInfo) -> ! {
    if let Some(location) = info.location() {
        println!(
            "[user] Panicked at {}:{} {}",
            location.file(),
            location.line(),
            info.message()
        );
    } else {
        println!("[user] Panicked: {}", info.message());
    }
    exit(-1);
}
