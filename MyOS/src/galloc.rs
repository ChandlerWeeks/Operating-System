//! # COSC562 Fall 2026 Operating System
//!
//! Allows for allocating and deallocating memory to the heap
//!
//! Jason Weeks - jweeks12
//! September 13th, 2026


/// WARNING: There is currently no heap allocator. Returns null until a heap is made.  

use core::alloc::GlobalAlloc;
use core::ptr::null_mut;
use crate::mem::{HeapAllocation, HEAP_CHUNK_SIZE};
use core::alloc::Layout;
use crate::data::kdata;

struct OSGlobalAlloc;

// interface (traits)
unsafe impl GlobalAlloc for OSGlobalAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // Rust tells us the number of bytes it wants in `layout`. However, we can only
        // hand out multiples of the chunk size, so we need to align it. The following formula
        // aligns if we cannot guarantee the chunk size is a power of two. It's a tad bit
        // more computationally expensive, but it gives us more chunk size options.
        let num_chunks = (layout.size() + HEAP_CHUNK_SIZE - 1) / HEAP_CHUNK_SIZE;
        // The kdata() function locks the heap Mutex and allows us to
        // access it.
        match kdata().with_heap_allocator(|heap| heap.nalloc_zeroed(num_chunks)) {
            None => null_mut(),
            Some(mut allocation) => {
                // Since this is using the global allocator, deallocation will be handled by it.
                // Leaking here prevents the "Possible leak" warning. Rust will manage the lifetime
                // of this allocation after we return it.
                unsafe {
                    allocation.leak();
                }
                // If we pick a chunk size where `layout.align()` always succeeds, we don't need
                // to align the returned memory address.
                allocation.as_mut_ptr()
            }
        }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let addr = ptr as usize;
        let num_chunks = (layout.size() + HEAP_CHUNK_SIZE - 1) / HEAP_CHUNK_SIZE;
        // We don't retain the original allocation, so we need to create a new one and deallocate it.
        // This is safe because we know the address and number of chunks are valid.
        let mut allocation = HeapAllocation::new(addr, num_chunks);
        kdata().with_heap_allocator(|heap| heap.dealloc(&mut allocation));
    }
}

#[global_allocator]
static MY_ALLOCATOR: OSGlobalAlloc = OSGlobalAlloc;
