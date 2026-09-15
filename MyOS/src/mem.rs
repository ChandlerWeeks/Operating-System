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
