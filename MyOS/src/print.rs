

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

pub struct DebugOutput;

impl core::fmt::Write for DebugOutput {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        for byte in s.bytes() {
            crate::sbi::debug_write_char(byte);
            if byte == b'\n' {
                crate::sbi::debug_write_char(b' ');
                crate::sbi::debug_write_char(b' ');
            }
        }
        Ok(())
    }
}

#[macro_export]
macro_rules! debugln
{
    // If we don't provide any arguments to `debugln!()`, then just
    // print out the file and line number.
    () => ({
        const LINE: u32 = line!();
        const FILE: &'static str = $crate::print::file_name(file!());
        use core::fmt::Write;
        let mut x = $crate::print::DebugOutput;
        let _ = write!(x, "[\x1b[96m{:<12} @ {:>4}\x1B[0m]", FILE, LINE);
        $crate::sbi::debug_write_char(b'\n');
    });
    // This matches one or more arguments
    ($($args:tt)+) => ({
        const LINE: u32 = line!();
        const FILE: &'static str = $crate::print::file_name(file!());
        use core::fmt::Write;
        let mut x = $crate::print::DebugOutput;
        let _ = write!(x, "[\x1b[96m{:<12} @ {:>4}\x1B[0m]\n", FILE, LINE);
        let _ = write!(x, $($args)+);
        $crate::sbi::debug_write_char(b'\n');
    });
}


pub const fn file_name(path: &str) -> &str {
    let bytes = path.as_bytes();
    let mut i = bytes.len() - 1;
    while i > 0 {
        if bytes[i] == b'/' {
            return unsafe {
                core::str::from_utf8_unchecked(
                    bytes.split_at(i + 1).1
                )   
            }
        }
        i -= 1
    }
    path
}
