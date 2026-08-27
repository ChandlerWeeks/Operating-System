//! # COSC562 Fall 2026 Operating System
//!
//! <Your Name>
//! 17 August 2026
#![no_std]
#![no_main]

#[macro_use]
extern crate alloc;

// ============================================================================
// Language Panic Handler
// ============================================================================
#[panic_handler]
fn panic(info: &core::panic::PanicInfo<'_>) -> ! {
    print!("[\x1b[91mOS PANIC\x1b[0m]");
    if let Some(loc) = info.location() {
        print!(" {}@{}.{}", loc.file(), loc.line(), loc.column());
    }
    println!(": {}", info.message());
    // Busy loop to prevent panic!() from returning.
    loop {
        unsafe {
            core::arch::asm!("wfi");
        }
    }
}

// ============================================================================
// Entry Point (_start and main)
// ============================================================================

/// # Entry point
/// 
/// Coming from the Limine boot loader, we are in S-mode with the MMU turned on.
/// We do not have: {`stvec, sie, sstatus`}.
#[unsafe(no_mangle)]
unsafe extern "C" fn _start() -> ! {
    unsafe extern "C" {
        #[link_name = "__global_pointer$"]
        unsafe static __GLOBAL_POINTER: u8;
    }

    unsafe {
        core::arch::asm!(
            "la gp, {address}",
            address = sym __GLOBAL_POINTER,
            options(nostack, nomem),
        );
    }

    main();
    sbi::hart_stop();
    panic!("About to return from _start()");
}

fn main() {
    // Write startup code here
    println!("Hello World");
    loop {
        unsafe {
            core::arch::asm!("wfi");
         }
    }
}

pub struct DebugOutput;

impl core::fmt::Write for DebugOutput {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        for byte in s.bytes() {
            $crate::sbi::debug_write_char(byte);
            if byte == b'\n' {
                $crate::sbi::debug_write_char(b' ');
                $crate::sbi::debug_write_char(b' ');
            }
        }

    }
}

// ============================================================================
// Modules (basically #include for Rust files)
// ============================================================================

pub mod print;
pub mod sbi;
pub mod galloc; 
