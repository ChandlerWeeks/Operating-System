use core::slice::from_raw_parts;
use core::slice::from_raw_parts_mut;
use core::ptr::null_mut;

const PAGE_SIZE: usize = 4096;

#[derive(Debug)]
struct FreeListPageNode<const N: usize> {
    next: *mut FreeListPageNode<N>,
}

#[derive(Debug)]
pub struct FreeListAllocation<const N: usize> {
    addr: usize
}

// Allocation methods
impl<const N: usize> FreeListAllocation<N> {
    /// Create a new allocation with the given address.
    ///
    /// # Safety
    ///
    /// The address is not checked against an allocator.
    pub const fn new(addr: usize) -> Self {
        Self {
            addr
        }
    }

    /// Get the starting (low) address of an allocation.
    pub fn start_addr(&self) -> usize {
        self.addr
    }

    /// Get the ending (high) address of an allocation.
    pub fn end_addr(&self) -> usize {
        self.start_addr() + self.len()
    }

    /// Get the number of bytes in this allocation.
    pub fn len(&self) -> usize {
        N
    }

    /// Is the allocation address valid?
    ///
    /// # Safety
    ///
    /// Only checks if addr is not null.
    pub fn is_valid(&self) -> bool {
        self.addr != 0
    }

    /// Is the allocation address invalid?
    ///
    /// # Safety
    ///
    /// Only checks if addr is null
    pub fn is_invalid(&self) -> bool {
        self.addr == 0
    }

    /// Invalid an allocation
    ///
    /// # Safety
    ///
    /// Allocation and address must not be used afterward.
    pub fn invalidate(&mut self) {
        self.addr = 0;
    }

    /// Get a pointer to the start of this allocation.
    pub fn as_ptr(&self) -> *const u8 {
        self.addr as *const u8
    }

    /// Get the pointer to the start of this allocation + the given
    /// offset. Checks if the start + offset is within length of the
    /// allocation.
    ///
    /// * `None` if offset is past the allocation.
    /// * `Some(ptr)` if all is ok.
    pub fn as_ptr_offset(&self, offset: usize) -> Option<*const u8> {
        if offset >= self.len() {
            return None;
        }
        Some(unsafe { self.as_ptr().add(offset) })
    }

    /// Get a slice of bytes from the start of this allocation.
    pub fn as_ref(&self) -> Option<&[u8]> {
        if self.is_invalid() {
            return None;
        }
        Some(unsafe { from_raw_parts(self.as_ptr(), N) })
    }

    /// Get a mutable pointer to this allocation.
    pub fn as_mut_ptr(&self) -> *mut u8 {
        self.addr as *mut u8
    }

    /// Get the mutable pointer to the start of this allocation + the given
    /// offset. Checks if the start + offset is within length of the
    /// allocation.
    ///
    /// * `None` if offset is past the allocation.
    /// * `Some(ptr)` if all is ok.
    pub fn as_mut_ptr_offset(&self, offset: usize) -> Option<*mut u8> {
        if offset >= self.len() {
            return None;
        }
        Some(unsafe { self.as_mut_ptr().add(offset) })
    }

    /// Get a mutable slice of this allocation.
    pub fn as_mut(&mut self) -> Option<&mut [u8]> {
        if self.is_invalid() {
            return None;
        }
        Some(unsafe { from_raw_parts_mut(self.as_mut_ptr(), N) })
    }

    /// Clear the allocation to all 0s.
    pub fn clear(&mut self) {
        if let Some(bytes) = self.as_mut() {
            bytes.fill(0);
        }
    }
}

pub struct FreeListAllocator<const N: usize> {
    head: *mut FreeListPageNode<N>,
    free_count: usize,
    total_count: usize,
}
/// # SAFETY
///
/// * The caller must ensure that it is safe to send the allocator to another thread.
/// * Typically that means this will be locked behind a Mutex when referred.
unsafe impl<const N: usize> Send for FreeListAllocator<N> {}

impl<const N: usize> FreeListAllocator<N> {
    // Our rounding calculation requires N to be a power of two.
    const ASSERT_POW2: () = assert!(N.is_power_of_two(), "N is not a power of two!");
    /// Create an empty allocator. Call `add_region` to feed it memory.
    pub const fn new() -> Self {
        // ASSERT_POW2 is compile-time constant, so it is never called. This let binding
        // ensures that it will be placed in the `new()` method and actually be called.
        let _ = Self::ASSERT_POW2;
        Self {
            head: null_mut(),
            free_count: 0,
            total_count: 0,
        }
    }
}

/// Align the given value up to the next power of two
const fn align_up_pot(value: usize, pot: usize) -> usize {
    debug_assert!(pot.is_power_of_two(), "Alignment is not a power of two!");
    (value + pot - 1) & !(pot - 1)
}
/// Align the given value down to the previous power of two
const fn align_down_pot(value: usize, pot: usize) -> usize {
    debug_assert!(pot.is_power_of_two(), "Alignment is not a power of two!");
    value & !(pot - 1)
}

impl<const N: usize> core::fmt::Debug for FreeListAllocator<N> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "FreeListAllocator<{}> {{ head: 0x{:08x}, free_count: {} ({} bytes), total_count: {} ({} bytes) }}",
            N,
            self.head_addr(),
            self.free_count(),
            self.free_bytes(),
            self.total_count(),
            self.total_bytes()
        )
    }
}

impl<const N: usize> FreeListAllocator<N> {
    /// Add a region of usable memory to the allocator.
    ///
    /// `start` and `end` are addresses. They will be aligned
    /// inward to page boundaries (start rounds up, end rounds down).
    ///
    /// # Safety
    /// The caller must guarantee that the region is truly free, mapped,
    /// and will not be used by anything else.
    pub unsafe fn add_region(&mut self, start: usize, end: usize) {
        let mut addr: usize = align_up_pot(start, N);
        let end_rounded: usize = align_down_pot(end, N);

        while addr < end_rounded {
            let node = addr as *mut FreeListPageNode<N>;
            unsafe { (*node).next = self.head; }
            self.head = node;
            self.free_count += 1;
            self.total_count += 1;

            addr += N;
        }
    }

    /// Allocate a single frame.
    pub fn alloc(&mut self) -> Option<FreeListAllocation<N>> {
        if self.head.is_null() {
            return None;
        } 
        self.free_count -= 1;
        let node = self.head;
        unsafe { self.head = (*node).next }
        
        Some(FreeListAllocation::new(node as usize))
    }
    /// Allocate a single frame and zero it.
    pub fn alloc_zeroed(&mut self) -> Option<FreeListAllocation<N>> {
        if let Some(mut a) = self.alloc() {
            a.clear();
            Some(a)
        } else {
            None
        }
    }
    /// Free an allocation, returning it to the pool.
    ///
    /// # Safety
    /// The caller must guarantee:
    /// - `addr` is N-aligned
    /// - `addr` was previously returned by `alloc`
    /// - `addr` is not currently in use or double-freed
    pub unsafe fn dealloc(&mut self, allocation: &mut FreeListAllocation<N>) {
        assert!(allocation.is_valid(), "cannot deallocate an invalid allocation");
        let node = allocation.start_addr() as *mut FreeListPageNode<N>;

        unsafe {
            (*node).next = self.head;
        }
        self.head = node;
        self.free_count += 1;
        allocation.invalidate();
    }

    /// Clear a frame to zero before deallocating it. This is to avoid having stale data
    /// sitting in the free list.
    pub unsafe fn dealloc_zeroed(&mut self, allocation: &mut FreeListAllocation<N>) {
        // The `clear()` method checks that the allocation is valid.
        allocation.clear();
        unsafe {
            self.dealloc(allocation);
        }
    }
    /// Number of free pages available.
    pub fn free_count(&self) -> usize {
        self.free_count
    }

    /// Total pages ever added to this allocator.
    pub fn total_count(&self) -> usize {
        self.total_count
    }

    /// Free memory in bytes.
    pub fn free_bytes(&self) -> usize {
         self.free_count() * N
    }

    /// Total memory in bytes.
    pub fn total_bytes(&self) -> usize {
        self.total_count() * N
    }

    /// Head address
    pub fn head_addr(&self) -> usize {
        self.head as usize
    }
}

/// Number of bytes in one kernel heap chunk.
///
/// Eight bytes matches the normal alignment limit for ordinary RV64 data.
/// Code that asks for a larger alignment must be rejected by the global
/// allocator before it reaches this allocator.
pub const HEAP_CHUNK_SIZE: usize = 8;

/// One fixed-size chunk in the kernel heap pool.
#[repr(transparent)]
struct Chunk<const N: usize> {
    data: [u8; N],
}

/// A single allocation from the chunk allocator.
///
/// This allocation stores its start address and its number of chunks. The
/// `leaked` flag prevents a false leak warning when the global allocator takes
/// ownership of the raw pointer.
#[derive(Debug)]
pub struct ChunkAllocation<const N: usize> {
    addr: usize,
    num: usize,
    leaked: bool,
}

// Allocation methods
impl<const N: usize> ChunkAllocation<N> {
    /// Create a new allocation with the given address and chunk count.
    ///
    /// # Safety
    ///
    /// The address and count are not checked against an allocator.
    pub const fn new(addr: usize, num: usize) -> Self {
        Self {
            addr,
            num,
            leaked: false,
        }
    }

    /// Get the starting address of this allocation.
    pub fn start_addr(&self) -> usize {
        self.addr
    }

    /// Get the ending address of this allocation.
    pub fn end_addr(&self) -> usize {
        self.start_addr() + self.len()
    }

    /// Get the number of chunks in this allocation.
    pub fn num_chunks(&self) -> usize {
        self.num
    }

    /// Get the number of bytes in this allocation.
    pub fn len(&self) -> usize {
        self.num * N
    }

    /// Is this allocation valid?
    pub fn is_valid(&self) -> bool {
        self.addr != 0
    }

    /// Is this allocation invalid?
    pub fn is_invalid(&self) -> bool {
        !self.is_valid()
    }

    /// Invalidate this allocation after it is freed.
    pub fn invalidate(&mut self) {
        self.addr = 0;
        self.num = 0;
        self.leaked = false;
    }

    /// Mark this allocation as owned by the global allocator.
    ///
    /// # Safety
    ///
    /// The caller must arrange for the allocation to be freed later.
    pub unsafe fn leak(&mut self) {
        self.leaked = true;
    }

    /// Get a read-only pointer to the allocation start.
    pub fn as_ptr(&self) -> *const u8 {
        self.addr as *const u8
    }

    /// Get a pointer at an offset from the allocation start.
    pub fn as_ptr_offset(&self, offset: usize) -> Option<*const u8> {
        if self.is_invalid() || offset >= self.len() {
            return None;
        }
        Some(unsafe { self.as_ptr().add(offset) })
    }

    /// Get a byte slice for this allocation.
    pub fn as_ref(&self) -> Option<&[u8]> {
        if self.is_invalid() {
            return None;
        }
        Some(unsafe { from_raw_parts(self.as_ptr(), self.len()) })
    }

    /// Get a mutable pointer to the allocation start.
    pub fn as_mut_ptr(&mut self) -> *mut u8 {
        self.addr as *mut u8
    }

    /// Get a mutable pointer at an offset from the allocation start.
    pub fn as_mut_ptr_offset(&mut self, offset: usize) -> Option<*mut u8> {
        if self.is_invalid() || offset >= self.len() {
            return None;
        }
        Some(unsafe { self.as_mut_ptr().add(offset) })
    }

    /// Get a mutable byte slice for this allocation.
    pub fn as_mut(&mut self) -> Option<&mut [u8]> {
        if self.is_invalid() {
            return None;
        }
        Some(unsafe { from_raw_parts_mut(self.as_mut_ptr(), self.len()) })
    }

    /// Clear every byte in this allocation.
    pub fn clear(&mut self) {
        if let Some(bytes) = self.as_mut() {
            bytes.fill(0);
        }
    }
}

impl<const N: usize> Drop for ChunkAllocation<N> {
    fn drop(&mut self) {
        if self.is_valid() && !self.leaked {
            crate::debugln!("!!!! POSSIBLE MEMORY LEAK !!!!  {:?}", self);
        }
    }
}

/// Allocates contiguous fixed-size chunks from one heap range.
///
/// The book stores one bit per chunk. A set bit means allocated. A clear bit
/// means free. Both the book and the pool live in the heap range supplied to
/// `new`, so this allocator needs no extra memory.
pub struct ChunkAllocator<const N: usize> {
    book: &'static mut [u8],
    pool: &'static mut [Chunk<N>],
}

impl<const N: usize> ChunkAllocator<N> {
    // The pool start must be aligned by the chunk size.
    const ASSERT_POW2: () = assert!(N.is_power_of_two(), "N is not a power of two!");

    /// Create an allocator over one contiguous, writable heap range.
    ///
    /// # Safety
    ///
    /// The caller must guarantee that this range is mapped, writable, not
    /// shared with other code, and valid for the life of the allocator.
    pub unsafe fn new(start: usize, total: usize) -> Self {
        let _ = Self::ASSERT_POW2;
        assert!(start != 0, "heap start address is null");
        assert!(start & (N - 1) == 0, "heap start is not chunk aligned");

        // Start with a conservative chunk count. One bit in `book` tracks
        // each chunk. The loop below accounts for alignment padding.
        let denominator = N.saturating_mul(8).saturating_add(1);
        let mut num_chunks = total.saturating_mul(8) / denominator;
        let mut book_len = num_chunks.saturating_add(7) / 8;
        let mut pool_offset = align_up_pot(book_len, N);

        while num_chunks > 0
            && (pool_offset > total || num_chunks > (total - pool_offset) / N)
        {
            num_chunks -= 1;
            book_len = num_chunks.saturating_add(7) / 8;
            pool_offset = align_up_pot(book_len, N);
        }

        let book = unsafe { from_raw_parts_mut(start as *mut u8, book_len) };
        book.fill(0);

        let pool_start = start
            .checked_add(pool_offset)
            .expect("kernel heap pool address overflow");
        let pool = unsafe {
            from_raw_parts_mut(pool_start as *mut Chunk<N>, num_chunks)
        };

        Self { book, pool }
    }

    /// Is the chunk at `which` allocated?
    fn is_allocated(&self, which: usize) -> bool {
        debug_assert!(which < self.pool.len());
        let byte = which / 8;
        let bit = which % 8;
        self.book[byte] & (1 << bit) != 0
    }

    /// Set or clear the allocation bit for one chunk.
    fn set_allocated(&mut self, which: usize, allocated: bool) {
        debug_assert!(which < self.pool.len());
        let byte = which / 8;
        let bit = which % 8;
        let mask = 1 << bit;

        if allocated {
            self.book[byte] |= mask;
        } else {
            self.book[byte] &= !mask;
        }
    }

    /// Allocate `num` contiguous chunks with a first-fit search.
    pub fn nalloc(&mut self, num: usize) -> Option<ChunkAllocation<N>> {
        if num == 0 || num > self.pool.len() {
            return None;
        }

        let mut first = 0;
        let mut run = 0;

        for which in 0..self.pool.len() {
            if self.is_allocated(which) {
                run = 0;
                continue;
            }

            if run == 0 {
                first = which;
            }
            run += 1;

            if run == num {
                for allocated in first..first + num {
                    self.set_allocated(allocated, true);
                }
                let addr = self.pool[first].data.as_mut_ptr() as usize;
                return Some(ChunkAllocation::new(addr, num));
            }
        }

        None
    }

    /// Allocate `num` contiguous chunks and clear them to zero.
    pub fn nalloc_zeroed(&mut self, num: usize) -> Option<ChunkAllocation<N>> {
        let mut allocation = self.nalloc(num)?;
        allocation.clear();
        Some(allocation)
    }

    /// Free a chunk allocation.
    ///
    /// # Safety
    ///
    /// The allocation must have come from this allocator and must not have
    /// been freed already.
    pub unsafe fn dealloc(&mut self, allocation: &mut ChunkAllocation<N>) {
        assert!(allocation.is_valid(), "cannot deallocate an invalid allocation");

        let pool_start = self.pool.as_mut_ptr() as usize;
        assert!(allocation.start_addr() >= pool_start, "allocation is below heap pool");
        let offset = allocation.start_addr() - pool_start;
        assert!(offset % N == 0, "allocation is not chunk aligned");

        let first = offset / N;
        assert!(first <= self.pool.len(), "allocation starts past heap pool");
        assert!(
            allocation.num_chunks() <= self.pool.len() - first,
            "allocation extends past heap pool"
        );

        for which in first..first + allocation.num_chunks() {
            self.set_allocated(which, false);
        }
        allocation.invalidate();
    }

    /// Get the total number of chunks in the heap.
    pub fn total_chunks(&self) -> usize {
        self.pool.len()
    }

    /// Get the number of allocated chunks in the heap.
    pub fn allocated_chunks(&self) -> usize {
        (0..self.pool.len())
            .filter(|&which| self.is_allocated(which))
            .count()
    }

    /// Get the number of free chunks in the heap.
    pub fn free_chunks(&self) -> usize {
        self.total_chunks() - self.allocated_chunks()
    }

    /// Get the total bytes available for allocations.
    pub fn total_bytes(&self) -> usize {
        self.total_chunks() * N
    }

    /// Get the number of free bytes in the heap.
    pub fn free_bytes(&self) -> usize {
        self.free_chunks() * N
    }
}

// ==============================================================================
// Kernel Page Allocator Types
// ==============================================================================
pub type PageAllocator = FreeListAllocator<PAGE_SIZE>;
pub type PageAllocation = FreeListAllocation<PAGE_SIZE>;

// ==============================================================================
// Kernel Heap Allocator Types
// ==============================================================================
pub type HeapAllocator = ChunkAllocator<HEAP_CHUNK_SIZE>;
pub type HeapAllocation = ChunkAllocation<HEAP_CHUNK_SIZE>;
