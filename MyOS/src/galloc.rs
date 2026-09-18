//! # COSC562 Fall 2026 Operating System
//!
//! Allows for allocating and deallocating memory to the heap
//!
//! Jason Weeks - jweeks12
//! September 13th, 2026


/// Returns null until the kernel initializes its heap allocator.

use core::alloc::GlobalAlloc;
use core::alloc::Layout;
use core::ptr::null_mut;

use crate::data::try_kdata;
use crate::mem::{HeapAllocation, HEAP_CHUNK_SIZE};

struct OSGlobalAlloc;

/// Return the number of heap chunks required by a Rust allocation.
fn chunks_for(layout: Layout) -> Option<usize> {
    if layout.size() == 0 || layout.align() > HEAP_CHUNK_SIZE {
        return None;
    }

    layout
        .size()
        .checked_add(HEAP_CHUNK_SIZE - 1)
        .map(|size| size / HEAP_CHUNK_SIZE)
}

// interface (traits)
unsafe impl GlobalAlloc for OSGlobalAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let Some(num_chunks) = chunks_for(layout) else {
            return null_mut();
        };
        let Some(data) = try_kdata() else {
            return null_mut();
        };

        match data.with_heap_allocator(|heap| heap.nalloc_zeroed(num_chunks)) {
            None => null_mut(),
            Some(mut allocation) => {
                // The global allocator takes ownership of this raw pointer.
                unsafe { allocation.leak(); }
                allocation.as_mut_ptr()
            }
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let Some(num_chunks) = chunks_for(layout) else {
            return;
        };
        let Some(data) = try_kdata() else {
            return;
        };

        let addr = ptr as usize;
        // The layout gives the same chunk count used by alloc.
        let mut allocation = HeapAllocation::new(addr, num_chunks);
        data.with_heap_allocator(|heap| unsafe { heap.dealloc(&mut allocation) });
    }
}

#[global_allocator]
static MY_ALLOCATOR: OSGlobalAlloc = OSGlobalAlloc;
