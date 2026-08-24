

pub struct KernelOut;
impl core::fmt::Write for KernelOut {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        for c in s.bytes() {
            crate::sbi::debug_write_char(c);
        }
        Ok(())
    }
}

#[macro_export]
macro_rules! print {
    ($($args:tt)+) => ({
        use core::fmt::Write;
        let _ = write!($crate::print::KernelOut, $($args)+);
    });
}

#[macro_export]
macro_rules! println {
    () => (
        print!("\r\n")
    );
    ($fmt:expr) => (
        print!(concat!($fmt, "\r\n"))
    );
    ($fmt:expr, $($args:tt)+) => (
        print!(concat!($fmt, "\r\n"), $($args)+)
    );
}

pub fn clear_screen() {
    println!("\x1b\x5b\x48\x1b\x5b\x32\x4a\x1b\x5b\x33\x4a");
}
