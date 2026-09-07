//! allocation and deallocation


/// WARNING: There is currently no heap allocator. Returns null until a heap is made.  

use core::alloc::GlobalAlloc;
use core::ptr::null_mut;

struct MyGlobalAllocator;

// interface (traits)
unsafe impl GlobalAlloc for MyGlobalAllocator {
    /// TODO: Allocate things once we have a heap
    unsafe fn alloc(&self, _layout: core::alloc::Layout) -> *mut u8 {
        null_mut()
    }
    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: core::alloc::Layout) {
        
    }
}

#[global_allocator]
static MY_ALLOCATOR: MyGlobalAllocator = MyGlobalAllocator;
