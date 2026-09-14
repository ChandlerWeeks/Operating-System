//! # COSC562 Fall 2026 Operating System
//!
//! Contains kernel configuration information, such as max_harts.
//!
//! Jason Weeks - jweeks12
//! September 13th, 2026

/// # Maximum number of supported harts
pub const MAX_HARTS: usize = 16;
/// # Kernel Heap Address
///
/// This is the virtual address where the kernel's heap starts. Since
/// we are a higher-half kernel, all kernel virtual addresses should
/// be higher-half too.
pub const KERNEL_HEAP_ADDR: usize = 0xffff_cafe_0000_0000;

/// This is the number of pages reserved for the kernel heap.
pub const NUM_HEAP_PAGES: usize = 1024;

/// # Kernel IO Addresses
///
/// Map IO addresses to a different virtual address prefix.
pub const KERNEL_IO_ADDR: usize = 0xffff_beef_0000_0000;
