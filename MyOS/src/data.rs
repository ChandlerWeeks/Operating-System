use crate::mem::HeapAllocator;
use crate::mem::PageAllocator;
use crate::sync::Mutex;
use crate::sync::OnceLock;
use crate::sv48::PageTableRoot;

#[unsafe(link_section = ".data.rust.forced")]
static KERNEL_DATA: OnceLock<KernelData> = OnceLock::new();

/// Get the global kernel data reference.
pub fn kdata() -> &'static KernelData {
    try_kdata().expect("Kernel data not initialized")
}

/// Get kernel data when initialization has finished.
pub fn try_kdata() -> Option<&'static KernelData> {
    KERNEL_DATA.get()
}

/// Publish the kernel data once during early boot.
pub fn init_kdata(data: KernelData) {
    assert!(KERNEL_DATA.set(data).is_ok(), "Kernel data already initialized");
}

pub struct KernelData {
    page_allocator: Mutex<PageAllocator>,
    heap_allocator: Mutex<HeapAllocator>,
    page_table_root: Mutex<PageTableRoot>,
}

impl KernelData {
    pub fn new(page_allocator: PageAllocator, heap_allocator: HeapAllocator, page_table_root: PageTableRoot) -> Self {
        Self {
            page_allocator: Mutex::new(page_allocator),
            heap_allocator: Mutex::new(heap_allocator),
            page_table_root: Mutex::new(page_table_root),
        }
    }

    /// Lock the page allocator for one short action.
    pub fn with_page_allocator<R>(&self, f: impl FnOnce(&mut PageAllocator) -> R) -> R {
        let mut page_allocator = self.page_allocator.lock();
        f(&mut page_allocator)
    }

    /// Lock the heap allocator for one short action.
    pub fn with_heap_allocator<R>(&self, f: impl FnOnce(&mut HeapAllocator) -> R) -> R {
        let mut heap_allocator = self.heap_allocator.lock();
        f(&mut heap_allocator)
    }

    /// Lock the page table root for one short action.
    pub fn with_page_table<R>(&self, f: impl FnOnce(&mut PageTableRoot) -> R) -> R {
        let mut page_table_root = self.page_table_root.lock();
        f(&mut page_table_root)
    }
}
