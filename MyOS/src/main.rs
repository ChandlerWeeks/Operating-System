//! # COSC562 Fall 2026 Operating System
//!
//! This serves to load the kernel.
//!
//! Jason Weeks - jweeks12
//! September 7th, 2026
#![no_std]
#![no_main]

use crate::print::clear_screen;

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
#[unsafe(no_mangle)]
unsafe extern "C" fn _start() -> ! {
    unsafe extern "C" {
        #[link_name = "__global_pointer$"]
        unsafe static __GLOBAL_POINTER: u8;
    }

    unsafe {
        /// set the global pointer
        core::arch::asm!(
            "la gp, {address}",
            address = sym __GLOBAL_POINTER,
            options(nostack, nomem),
        );
    }
    main();
    sbi::hart_stop();
}

fn main() {
    // Clear the screen, and test debugln!, println!, and power off the system.
    clear_screen();
    println!("Hello World");
    let (major, minor) = sbi::get_spec_version();
    println!("Major: {}, Minor: {}", major, minor);
    debugln!("Kernel activate beep boop");
    sbi::shutdown();
}

// ============================================================================
// Modules (basically #include for Rust files)
// ============================================================================

pub mod print;
pub mod sbi;
pub mod galloc;
pub mod limine;
pub mod config;
pub mod acpi; 
