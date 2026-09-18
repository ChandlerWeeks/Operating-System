//! Sv48 page-table types and operations.

use core::arch::asm;
use core::ops::{Index, IndexMut};

use crate::data::kdata;
use crate::limine::{pa_to_va, va_to_pa};
use crate::mem::{PageAllocation, PageAllocator};
use crate::riscv::{pte, satp_reg};

/// Sv48 uses 4 KiB page-table pages.
const PAGE_SIZE: usize = 4096;
const ENTRIES_PER_TABLE: usize = 512;
const PPN_MASK: usize = ((1usize << 44) - 1) << pte::PPN_0_BIT;

// ==============================================================================
// Memory Flags. These flags describe a requested mapping.
// ==============================================================================
const MEM_VALID: usize = 0;
const MEM_READ: usize = 1;
const MEM_WRITE: usize = 2;
const MEM_EXECUTE: usize = 3;
const MEM_USER: usize = 4;
const MEM_GLOBAL: usize = 5;
const MEM_ACCESSED: usize = 6;
const MEM_DIRTY: usize = 7;
const MEM_SUPER: usize = 8;
const MEM_HUGE: usize = 9;
const MEM_ENORMOUS: usize = 10;
const MEM_IO: usize = 11;
const NUM_FLAGS: usize = 12;

/// A kernel virtual address used by SV48 page-table code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VirtualAddress {
    address: usize,
}

impl VirtualAddress {
    /// Wrap a virtual address without changing its value.
    pub const fn new(address: usize) -> Self {
        Self { address }
    }

    /// Return the wrapped address as a usize.
    pub const fn as_usize(self) -> usize {
        self.address
    }
}

impl From<usize> for VirtualAddress {
    fn from(address: usize) -> Self {
        Self::new(address)
    }
}

/// Flags for a requested memory mapping.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct MemFlags {
    flags: usize,
}

impl MemFlags {
    /// Make a mapping with no flags.
    pub const fn new() -> Self {
        Self { flags: 0 }
    }

    /// Add one memory flag.
    pub fn set(&mut self, flag: usize) {
        assert!(flag < NUM_FLAGS);
        self.flags |= 1 << flag;
    }

    /// Return the stored memory flags.
    pub const fn get(&self) -> usize {
        self.flags
    }

    /// Add the valid flag.
    pub fn valid(mut self) -> Self {
        self.set(MEM_VALID);
        self
    }

    /// Add read permission.
    pub fn read(mut self) -> Self {
        self.set(MEM_READ);
        self
    }

    /// Add write permission.
    pub fn write(mut self) -> Self {
        self.set(MEM_WRITE);
        self
    }

    /// Add execute permission.
    pub fn execute(mut self) -> Self {
        self.set(MEM_EXECUTE);
        self
    }

    /// Allow user-mode access.
    pub fn user(mut self) -> Self {
        self.set(MEM_USER);
        self
    }

    /// Mark the mapping global.
    pub fn global(mut self) -> Self {
        self.set(MEM_GLOBAL);
        self
    }

    /// Mark the mapping accessed.
    pub fn accessed(mut self) -> Self {
        self.set(MEM_ACCESSED);
        self
    }

    /// Mark the mapping dirty.
    pub fn dirty(mut self) -> Self {
        self.set(MEM_DIRTY);
        self
    }

    /// Request a 2 MiB leaf at level 1.
    pub fn super_page(mut self) -> Self {
        self.set(MEM_SUPER);
        self
    }

    /// Request a 1 GiB leaf at level 2.
    pub fn huge_page(mut self) -> Self {
        self.set(MEM_HUGE);
        self
    }

    /// Request a 512 GiB leaf at level 3.
    pub fn enormous_page(mut self) -> Self {
        self.set(MEM_ENORMOUS);
        self
    }

    /// Request I/O memory attributes.
    pub fn io(mut self) -> Self {
        self.set(MEM_IO);
        self
    }

    /// Check one memory flag.
    pub const fn is_set(&self, flag: usize) -> bool {
        flag < NUM_FLAGS && self.flags & (1 << flag) != 0
    }

    pub const fn is_valid(&self) -> bool { self.is_set(MEM_VALID) }
    pub const fn is_readable(&self) -> bool { self.is_set(MEM_READ) }
    pub const fn is_writeable(&self) -> bool { self.is_set(MEM_WRITE) }
    pub const fn is_executable(&self) -> bool { self.is_set(MEM_EXECUTE) }
    pub const fn is_user(&self) -> bool { self.is_set(MEM_USER) }
    pub const fn is_global(&self) -> bool { self.is_set(MEM_GLOBAL) }
    pub const fn is_accessed(&self) -> bool { self.is_set(MEM_ACCESSED) }
    pub const fn is_dirty(&self) -> bool { self.is_set(MEM_DIRTY) }
    pub const fn is_super_page(&self) -> bool { self.is_set(MEM_SUPER) }
    pub const fn is_huge_page(&self) -> bool { self.is_set(MEM_HUGE) }
    pub const fn is_enormous_page(&self) -> bool { self.is_set(MEM_ENORMOUS) }
    pub const fn is_io(&self) -> bool { self.is_set(MEM_IO) }

    /// Make memory flags from a leaf entry.
    fn from_entry(entry: &PageTableEntry, level: usize) -> Self {
        let mut flags = Self::new().valid();
        if entry.is_readable() { flags = flags.read(); }
        if entry.is_writeable() { flags = flags.write(); }
        if entry.is_executable() { flags = flags.execute(); }
        if entry.is_user() { flags = flags.user(); }
        if entry.is_global() { flags = flags.global(); }
        if entry.is_accessed() { flags = flags.accessed(); }
        if entry.is_dirty() { flags = flags.dirty(); }
        if entry.is_io() { flags = flags.io(); }
        if level == 1 { flags = flags.super_page(); }
        if level == 2 { flags = flags.huge_page(); }
        if level == 3 { flags = flags.enormous_page(); }
        flags
    }
}

/// One Sv48 page-table entry.
#[derive(Debug, Default, Clone, Copy)]
pub struct PageTableEntry {
    entry: usize,
}

impl PageTableEntry {
    /// Make an invalid entry with no flags or physical page number.
    pub const fn new_empty() -> Self {
        Self { entry: 0 }
    }

    /// Return all raw bits in this page-table entry.
    pub const fn bits(&self) -> usize {
        self.entry
    }

    /// Return the raw 64-bit entry.
    pub const fn raw(&self) -> usize {
        self.entry
    }

    /// Replace all bits in this page-table entry.
    pub fn set_bits(&mut self, bits: usize) {
        self.entry = bits;
    }

    /// Overwrite the raw 64-bit entry.
    pub fn set_raw(&mut self, value: usize) {
        self.entry = value;
    }

    /// Return the page-aligned physical address in this entry.
    pub const fn physical_address(&self) -> usize {
        ((self.entry & PPN_MASK) >> pte::PPN_0_BIT) << 12
    }

    /// Return the physical address of this entry's next table or leaf.
    pub const fn next_paddr(&self) -> usize {
        self.physical_address()
    }

    /// Replace the physical address and keep the current flags.
    pub fn set_physical_address(&mut self, paddr: usize) {
        assert!(paddr & (PAGE_SIZE - 1) == 0);
        let encoded_ppn = (paddr >> 12) << pte::PPN_0_BIT;
        assert!(encoded_ppn & !PPN_MASK == 0);
        self.entry = (self.entry & !PPN_MASK) | encoded_ppn;
    }

    pub const fn is_valid(&self) -> bool { self.entry & pte::VALID != 0 }
    pub const fn is_readable(&self) -> bool { self.entry & pte::READ != 0 }
    pub const fn is_writeable(&self) -> bool { self.entry & pte::WRITE != 0 }
    pub const fn is_executable(&self) -> bool { self.entry & pte::EXECUTE != 0 }
    pub const fn is_user(&self) -> bool { self.entry & pte::USER != 0 }
    pub const fn is_global(&self) -> bool { self.entry & pte::GLOBAL != 0 }
    pub const fn is_accessed(&self) -> bool { self.entry & pte::ACCESSED != 0 }
    pub const fn is_dirty(&self) -> bool { self.entry & pte::DIRTY != 0 }

    /// Check whether this valid entry maps memory instead of a table.
    pub const fn is_leaf(&self) -> bool {
        self.is_readable() || self.is_writeable() || self.is_executable()
    }

    /// Check whether this entry requests I/O memory attributes.
    pub const fn is_io(&self) -> bool {
        (self.entry >> pte::PBMT_BIT) & 0b11 == pte::PBMT_IO
    }
}

/// One 4 KiB SV48 page table.
#[repr(C, align(4096))]
pub struct PageTable {
    entries: [PageTableEntry; ENTRIES_PER_TABLE],
}

const _: () = assert!(core::mem::size_of::<PageTable>() == PAGE_SIZE);
const _: () = assert!(core::mem::align_of::<PageTable>() == PAGE_SIZE);

impl PageTable {
    /// Make a page table with 512 invalid entries.
    pub const fn new_empty() -> Self {
        Self { entries: [PageTableEntry::new_empty(); ENTRIES_PER_TABLE] }
    }

    /// Get one entry.
    pub fn get_ref(&self, index: usize) -> &PageTableEntry {
        assert!(index < ENTRIES_PER_TABLE);
        &self.entries[index]
    }

    /// Get one entry for update.
    pub fn get_mut(&mut self, index: usize) -> &mut PageTableEntry {
        assert!(index < ENTRIES_PER_TABLE);
        &mut self.entries[index]
    }
}

impl Index<usize> for PageTable {
    type Output = PageTableEntry;

    fn index(&self, index: usize) -> &Self::Output {
        self.get_ref(index)
    }
}

impl IndexMut<usize> for PageTable {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        self.get_mut(index)
    }
}

/// The top-level page table for one SV48 address space.
pub struct PageTableRoot {
    addr: VirtualAddress,
}

impl PageTableRoot {
    /// Wrap the virtual address of an aligned root page table.
    ///
    /// # Safety
    ///
    /// The address must identify a mapped, writable page table.
    pub unsafe fn wrap_vaddr(virtual_address: usize) -> Self {
        assert!(virtual_address & (PAGE_SIZE - 1) == 0);
        Self { addr: VirtualAddress::from(virtual_address) }
    }

    /// Read SATP and wrap the active SV48 root page table.
    pub fn current() -> Self {
        let satp: usize;
        unsafe { asm!("csrr {value}, satp", value = out(reg) satp); }

        let mode = (satp >> satp_reg::MODE_BIT) & ((1 << satp_reg::MODE_SIZE) - 1);
        assert!(mode == satp_reg::MODE_SV48);
        let root_paddr = (satp & ((1usize << satp_reg::PPN_SIZE) - 1)) << 12;
        let root_vaddr = pa_to_va(root_paddr);

        // `pa_to_va` returns the address that the kernel can dereference.
        unsafe { Self::wrap_vaddr(root_vaddr) }
    }

    /// Return the virtual address of the root page table.
    pub const fn virtual_address(&self) -> usize {
        self.addr.as_usize()
    }

    /// Map with the global page allocator.
    pub fn map(&mut self, vaddr: usize, paddr: usize, flags: MemFlags) {
        self.map_unmanaged(vaddr, paddr, flags, || {
            kdata().with_page_allocator(|allocator| {
                allocator.alloc().expect("out of pages for a page table")
            })
        });
    }

    /// Map one virtual range with a caller-provided table-page allocator.
    pub fn map_unmanaged<A>(&mut self, vaddr: usize, paddr: usize, mflags: MemFlags, mut allocator: A)
    where
        A: FnMut() -> PageAllocation,
    {
        let level = level_from_flags(mflags);
        let page_size = page_size_for_level(level);
        assert!(vaddr & (page_size - 1) == 0);
        assert!(paddr & (page_size - 1) == 0);
        assert!(!mflags.is_writeable() || mflags.is_readable());

        let vpn = split_vpn(vaddr);
        let mut table = unsafe { Self::table_mut_at(self.virtual_address()) };

        for table_level in ((level + 1)..4).rev() {
            let next_table_paddr = {
                let entry = table.get_mut(vpn[table_level]);
                if !entry.is_valid() {
                    let mut new_page = allocator();
                    new_page.clear();
                    entry.set_bits(pte::VALID);
                    entry.set_physical_address(va_to_pa(new_page.start_addr()));
                }
                assert!(!entry.is_leaf(), "cannot descend through a leaf PTE");
                entry.physical_address()
            };

            table = unsafe { Self::table_mut_at(pa_to_va(next_table_paddr)) };
        }

        let leaf = table.get_mut(vpn[level]);
        assert!(!leaf.is_valid(), "virtual address already has a mapping");
        leaf.set_bits(mem_flags_to_pte_flags(&mflags));
        leaf.set_physical_address(paddr);
        crate::riscv::sfence_vma(vaddr);
    }

    /// Return the flags for a mapped virtual address.
    pub fn access(&self, vaddr: usize) -> Option<MemFlags> {
        let vpn = split_vpn(vaddr);
        let mut table = unsafe { Self::table_ref_at(self.virtual_address()) };

        for level in (0..4).rev() {
            let entry = table.get_ref(vpn[level]);
            if !entry.is_valid() {
                return None;
            }
            if entry.is_leaf() {
                return Some(MemFlags::from_entry(entry, level));
            }
            if level == 0 {
                return None;
            }
            table = unsafe { Self::table_ref_at(pa_to_va(entry.physical_address())) };
        }

        None
    }

    /// Translate a virtual address to a physical address.
    pub fn translate(&self, vaddr: usize) -> Option<usize> {
        let vpn = split_vpn(vaddr);
        let mut table = unsafe { Self::table_ref_at(self.virtual_address()) };

        for level in (0..4).rev() {
            let entry = table.get_ref(vpn[level]);
            if !entry.is_valid() {
                return None;
            }
            if entry.is_leaf() {
                let offset = vaddr & (page_size_for_level(level) - 1);
                return Some(entry.physical_address() + offset);
            }
            if level == 0 {
                return None;
            }
            table = unsafe { Self::table_ref_at(pa_to_va(entry.physical_address())) };
        }

        None
    }

    /// Clear the leaf PTE for one virtual address.
    pub fn unmap(&mut self, vaddr: usize) {
        let vpn = split_vpn(vaddr);
        let mut table = unsafe { Self::table_mut_at(self.virtual_address()) };

        for level in (0..4).rev() {
            let entry = table.get_mut(vpn[level]);
            if !entry.is_valid() {
                return;
            }
            if entry.is_leaf() {
                entry.set_bits(0);
                crate::riscv::sfence_vma(vaddr);
                return;
            }
            if level == 0 {
                return;
            }
            table = unsafe { Self::table_mut_at(pa_to_va(entry.physical_address())) };
        }
    }

    /// Free child page tables that this allocator owns.
    ///
    /// The root page is not freed because this type does not own it.
    /// The caller must stop using this address space before calling this method.
    pub fn free(mut self) {
        kdata().with_page_allocator(|allocator| {
            self.free_children(self.virtual_address(), 3, allocator);
        });
        crate::riscv::sfence_all();
    }

    /// Free owned branch pages below one page table.
    fn free_children(&mut self, table_vaddr: usize, level: usize, allocator: &mut PageAllocator) {
        let table = unsafe { Self::table_mut_at(table_vaddr) };

        for index in 0..ENTRIES_PER_TABLE {
            let entry = table.get_mut(index);
            if !entry.is_valid() {
                continue;
            }
            if entry.is_leaf() || level == 0 {
                entry.set_bits(0);
                continue;
            }

            let child_vaddr = pa_to_va(entry.physical_address());
            self.free_children(child_vaddr, level - 1, allocator);

            let mut allocation = PageAllocation::new(child_vaddr);
            unsafe { allocator.dealloc(&mut allocation); }
            entry.set_bits(0);
        }
    }

    /// View an HHDM or kernel virtual address as a page table.
    unsafe fn table_ref_at<'a>(address: usize) -> &'a PageTable {
        unsafe { &*(address as *const PageTable) }
    }

    /// View an HHDM or kernel virtual address as a mutable page table.
    unsafe fn table_mut_at<'a>(address: usize) -> &'a mut PageTable {
        unsafe { &mut *(address as *mut PageTable) }
    }
}

/// Split an SV48 virtual address into four VPN indexes.
const fn split_vpn(vaddr: usize) -> [usize; 4] {
    [
        (vaddr >> 12) & 0x1ff,
        (vaddr >> 21) & 0x1ff,
        (vaddr >> 30) & 0x1ff,
        (vaddr >> 39) & 0x1ff,
    ]
}

/// Get the leaf level selected by the memory flags.
fn level_from_flags(flags: MemFlags) -> usize {
    let page_sizes = flags.is_super_page() as usize
        + flags.is_huge_page() as usize
        + flags.is_enormous_page() as usize;
    assert!(page_sizes <= 1);

    if flags.is_enormous_page() {
        3
    } else if flags.is_huge_page() {
        2
    } else if flags.is_super_page() {
        1
    } else {
        0
    }
}

/// Return the mapping size for one SV48 leaf level.
const fn page_size_for_level(level: usize) -> usize {
    assert!(level < 4);
    PAGE_SIZE << (level * 9)
}

/// Convert generic memory flags to RISC-V PTE flag bits.
fn mem_flags_to_pte_flags(mflags: &MemFlags) -> usize {
    let mut pte_flags = pte::VALID;
    if mflags.is_readable() { pte_flags |= pte::READ; }
    if mflags.is_writeable() { pte_flags |= pte::WRITE; }
    if mflags.is_executable() { pte_flags |= pte::EXECUTE; }
    if mflags.is_user() { pte_flags |= pte::USER; }
    if mflags.is_global() { pte_flags |= pte::GLOBAL; }
    if mflags.is_accessed() { pte_flags |= pte::ACCESSED; }
    if mflags.is_dirty() { pte_flags |= pte::DIRTY; }
    if mflags.is_io() { pte_flags |= pte::PBMT_IO << pte::PBMT_BIT; }
    pte_flags
}
