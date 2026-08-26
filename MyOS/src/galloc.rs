use core::alloc::GlobalAlloc;
use core::ptr::null_mut;

struct MyGlobalAllocator;

// interface (traits)
unsafe impl GlobalAlloc for MyGlobalAllocator {
    unsafe fn alloc(&self, _layout: core::alloc::Layout) -> *mut u8 {
        null_mut()
    }
    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: core::alloc::Layout) {
        
    }
}

#[global_allocator]
static MY_ALLOCATOR: MyGlobalAllocator = MyGlobalAllocator;
