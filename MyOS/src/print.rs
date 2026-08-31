
/// # Kernel's Output
pub struct KernelOut;
/// implements the write trait from core::fmt for KernelOut, 
/// allowing the write system call to a place in the system.
impl core::fmt::Write for KernelOut {
    /// processes each byte (character) in a string and outputs it using the SBI
    ///
    /// Inputs:
    ///
    /// - `s`: the string to be output
    ///
    /// Return:
    ///
    /// - `result`: Successful run of the function
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        for c in s.bytes() {
            crate::sbi::debug_write_char(c);
        }
        Ok(())
    }
}

/// # Print Macro
///
/// Uses the write trait to output a line of text
///
/// Inputs:
///
/// - $args:tt: one or more tokens to output
#[macro_export]
macro_rules! print {
    ($($args:tt)+) => ({
        use core::fmt::Write;  
        let _ = write!($crate::print::KernelOut, $($args)+);
    });
}

/// # Print Line Macro
///
/// Uses the print macro to print a line of text
#[macro_export]
macro_rules! println {
    /// outputs a newline character and nothing else;
    () => (
        print!("\r\n")
    );
    /// outputs one string and a newline character
    ($fmt:expr) => (
        print!(concat!($fmt, "\r\n"))
    );
    /// ouputs a string and a newline character, allowing for formatted data within the string. 
    ($fmt:expr, $($args:tt)+) => (
        print!(concat!($fmt, "\r\n"), $($args)+)
    );
}

/// # Clear's the screen of previous text 
pub fn clear_screen() {
    println!("\x1b\x5b\x48\x1b\x5b\x32\x4a\x1b\x5b\x33\x4a");
}

/// Kernel output with auto indentation for line feed
pub struct DebugOutput;
/// implements the write trait from core::fmt for KernelOut, 
/// allowing the write system call to a place in the system.
impl core::fmt::Write for DebugOutput {
    /// processes each byte (character) in a string and outputs it using the SBI
    ///
    /// Inputs:
    ///
    /// - `s`: the string to be output
    ///
    /// Return:
    ///
    /// - `result`: Successful run of the function
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        for byte in s.bytes() {
            crate::sbi::debug_write_char(byte);
            /// auto indent on a new line for debugging lines.
            if byte == b'\n' {
                crate::sbi::debug_write_char(b' ');
                crate::sbi::debug_write_char(b' ');
            }
        }
        Ok(())
    }
}

/// Prints source code information with an optional message
#[macro_export]
macro_rules! debugln
{
    /// No message: print a file name and line number
    () => ({
        const LINE: u32 = line!();
        const FILE: &'static str = $crate::print::file_name(file!());
        use core::fmt::Write;
        let mut x = $crate::print::DebugOutput; let _ = write!(x, "[\x1b[96m{:<12} @ {:>4}\x1B[0m]", FILE, LINE);
        $crate::sbi::debug_write_char(b'\n');
    });
    /// Takes one or many arguements as a message: Prints a message and a file name with a line number
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

/// Reads and returns a file name from a directory path by finding the items after the last / 
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
