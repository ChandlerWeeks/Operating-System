# Memory Lab: Completion Guide

This guide gives the work order for the **Kernel Data, Symbols, and Memory**
lab. It is a guide, not a replacement for the lab page. It does not contain
new implementation code. Use the provided code on the lab page and the
repository source as the code reference.

## Goal

At the end of the lab, the kernel must own its usable memory. It must:

1. record linker section addresses;
2. allocate physical pages from all usable Limine memory regions;
3. manage a mapped kernel heap;
4. safely share the allocators and current page-table root;
5. change Sv48 page tables; and
6. map and clear the BSS before Rust uses data in it.

Do the work in this order. The later steps depend on the earlier steps.

## What the repository already provides

The current repository is at the Limine and ACPI lab stage. The memory-lab
files named on the course page do not exist yet. This is expected. Add or
complete them as the lab directs.

| Provided item | What it gives you | What it does **not** give you |
| --- | --- | --- |
| src/limine.rs | usable_memory_regions(), the HHDM, pa_to_va(), and va_to_pa() | A page allocator or page-table editor |
| src/config.rs | KERNEL_IO_ADDR, KERNEL_HEAP_ADDR, and NUM_HEAP_PAGES | Mappings at those virtual addresses |
| src/riscv.rs | SATP access support, sfence helpers, and PTE bit constants | Sv48 page-table structures or methods |
| src/sync.rs | OnceLock, Mutex, and lock guards | A KernelData instance or its access methods |
| riscv64gc-virt.ld | The six start/end symbol pairs needed by this lab | Rust types that expose those symbols |
| src/acpi.rs and src/main.rs | SPCR access and an XSDT walk that finds the UART physical address | An MMIO mapping for the UART |
| src/galloc.rs | The global allocator hook | A working heap allocator; it returns null now |

The linker defines these section pairs: text, rodata, data, BSS, trampoline,
and the full kernel. The BSS starts page-aligned in the current linker script.
It is a PT_NULL, NOLOAD section. Its bytes are not loaded or mapped by Limine.

Before you call code from sync.rs or riscv.rs, make those files active modules
in main.rs. Also add the new lab modules there. Add one module at a time and
run a source check often.

## Work order

### 1. Expose linker sections first

Create src/symbols.rs. Implement the provided linker_section! macro and use it
for Text, Rodata, Data, Bss, Trampoline, and Kernel.

For every section type, the required meaning is the same:

- start() returns the linker start address.
- end() returns the exclusive end address.
- size() is end minus start and must handle an empty section.
- contains(address) tests the half-open range [start, end).
- The byte-slice methods view exactly that range.

Use the symbol names from the current linker script. Do not add an Autoexec
section type: it is not a required item for this lab. Add
print_elf_sections() as a diagnostic. Use it once during early work to check
the order and sizes of the sections.

### 2. Build the page allocator

Create src/mem.rs and complete the provided free-list allocator types. This
allocator manages fixed-size blocks. For the page alias, one block is one
4,096-byte page.

The key design point is that Limine can return several usable regions. They do
not form one continuous region. Add each usable region to the same free list.
The free-list node lives in a free page, so the allocator does not need a
separate heap for its bookkeeping.

Use HHDM virtual addresses when the kernel reads or writes a free page. Keep
the distinction clear:

- The Limine memory map reports physical base addresses and lengths.
- The free-list allocation returned to kernel code is an address that the
  kernel can access.
- A page-table entry stores a physical address. Convert an allocated page
  address with the provided va_to_pa() before you place it in a PTE.

Complete the allocation handle before you use it elsewhere. It needs valid and
invalid states, its start address, its fixed length, safe bounds checks for
offset methods, and a way to clear a valid allocation. Invalidate an allocation
only when its address must no longer be used.

For the allocator itself, verify these cases before you continue:

- A region that is not page-aligned must not create a partly valid page.
- A region smaller than one complete page adds no page.
- Allocation from an empty list fails without dereferencing a null node.
- Freeing an allocation returns it to the list exactly once.
- Counts report free pages and total managed pages correctly.

The allocator has raw pointers. It may be marked Send only because later code
keeps it behind the supplied lock. Do not use it without that lock after shared
execution can start.

### 3. Build the heap allocator and connect the global hook

Complete the provided chunk allocator in src/mem.rs. It manages blocks inside
one continuous virtual heap range. It is different from the page allocator:
the page allocator obtains 4 KiB backing pages, while the chunk allocator
serves allocations with different sizes and alignments from those mapped pages.

Make the provided aliases unambiguous:

- PageAllocation and PageAllocator refer to 4 KiB free-list objects.
- HeapAllocation and HeapAllocator refer to the chunk allocator objects.

Do not use Box, Vec, String, or another heap-backed type during this step.
src/galloc.rs cannot allocate until step 6 completes. After the heap allocator
is stored in kernel data, make the GlobalAlloc implementation delegate
allocation and deallocation to it. Handle a layout that cannot fit; do not
return an invalid pointer.

### 4. Define one global kernel-data owner

Create src/data.rs. It owns the page allocator, heap allocator, and the current
PageTableRoot. Store the single KERNEL_DATA instance in the provided OnceLock
type.

Put this static in a writable .data.* linker section. A normal immutable Rust
static can go in read-only data. Limine maps read-only data without write
permission. A later write to an unforced OnceLock causes a page fault.

Place each mutable resource behind its own mutex. Provide short closure-based
access methods. A caller should borrow an allocator or the page-table root only
for the closure body. The guard then drops at the end of that body. Do not give
callers a long-lived mutable reference to a shared allocator.

The published page uses two names for the heap closure method:
with_heap_allocator in Submission and with_kernel_heap in the heap text. Use
the name required by the starter or grading interface that you receive. If
there is no additional starter interface, use with_heap_allocator, because it
is the name in the submission checklist. Keep one canonical operation; do not
make two independent heap locks.

Add a small helper that returns initialized kernel data or stops with a clear
kernel error. Do not let normal code access an uninitialized OnceLock.

### 5. Complete the Sv48 structures and operations

Create src/sv48.rs from the provided material. First complete MemFlags. Its
methods describe the requested access and page size. They are not PTE bits by
themselves. Convert them to the RISC-V PTE bit positions when you write a leaf
entry. For memory-mapped I/O, use the provided I/O PBMT value.

Then complete these structures and operations:

- PageTableEntry reads and changes one 64-bit entry.
- PageTable contains 512 entries and supports checked entry access.
- PageTableRoot identifies the current root and performs map, unmap,
  translate, access, and free.

The current root already exists. Read SATP, extract its physical page number,
form the physical root address, and convert that address to a usable virtual
address with pa_to_va(). Wrap that virtual address as the page-table root. Do
not treat the SATP physical address as a directly dereferenceable pointer.

For map, split the virtual address into its four Sv48 VPN fields. Walk from the
root down to the requested leaf level. If an intermediate entry is invalid,
allocate and clear one page-table page. Store that new page's **physical** PPN
in the branch PTE. A branch PTE is valid but has no read, write, or execute
bit. A leaf PTE has the requested physical PPN and access bits.

Use the actual physical address of the newly allocated page when you create a
new branch PTE. Keep the separate physical-address variable in scope. Do not
reuse an unrelated address while calculating that PPN.

Support ordinary, super, huge, and enormous leaves as the supplied flags
describe. Reject or clearly handle an attempt to descend through a leaf.
Make translate and access return None for an invalid walk. Unmap must clear
the leaf validity state and invalidate the affected translation. Flush the TLB
after a mapping change before the kernel relies on that change.

### 6. Initialize memory in the safe boot order

Change the early path in src/main.rs. The existing ACPI print-and-shutdown path
is useful for finding the UART, but it is not the final memory-lab boot path.
Use this order:

1. Create an empty page allocator.
2. Read every item from limine::usable_memory_regions() and add its usable
   pages to that allocator.
3. Read and wrap the current SATP page-table root.
4. Construct KernelData with the page allocator and root, then initialize
   KERNEL_DATA.
5. Map the BSS pages, flush the relevant kernel translations, and clear the
   BSS bytes.
6. Find and map the UART and map the RTC.
7. Allocate 1,024 backing pages, map them as the kernel heap, and install the
   heap allocator in KernelData.
8. Only now permit normal Rust heap allocation.

This order matters. The page-table walk needs the page allocator to create
missing intermediate tables. The BSS needs the page table. The heap needs the
page allocator and page table. The global allocator needs the heap.

### 7. Map BSS, I/O, and the heap

Use Bss::start() and Bss::size() from step 1. Round the BSS byte count up to a
page count. Map each BSS virtual page to its matching physical backing page. In
this linker layout, va_to_pa() gives the physical backing address for a
kernel-image virtual address. Give these mappings read and write access. Flush
after the batch, then clear the exact BSS byte range to zero. Do not clear BSS
before it is mapped.

Use the existing XSDT loop and Spcr reader to obtain the UART physical address.
Map it only when SPCR says that its address space is system memory. Map the RTC
physical address that the lab permits. Both device mappings use the kernel I/O
address range that starts at KERNEL_IO_ADDR; the supplied pa_to_va() gives that
virtual address for these low physical MMIO addresses. Give devices the I/O
memory attribute and do not give them execute permission.

For the heap, allocate exactly NUM_HEAP_PAGES pages from the page allocator.
Map their physical pages, in order, to the continuous range starting at
KERNEL_HEAP_ADDR. Give the range read and write access. Flush the mappings,
then create the chunk allocator over that virtual range and store it in kernel
data. Do not assume the source physical pages are continuous; the virtual heap
must be continuous, and each page mapping supplies that result.

## Completion checklist

Before submission, confirm all of these facts.

- [x] The six linker section types and print_elf_sections() exist.
- [ ] The page allocator accepts all usable Limine regions and handles no-memory
  cases safely.
- [ ] The heap allocator honors size and alignment, and the global allocator no
  longer always returns null after heap setup.
- [ ] There is one initialized KERNEL_DATA, with short, locked access to each
  mutable resource.
- [ ] Sv48 mapping, unmapping, translation, and access work for the page sizes
  required by the lab.
- [ ] The BSS is mapped, TLB-flushed, and zeroed before BSS data is used.
- [ ] UART and RTC are mapped in the I/O virtual range. The UART address comes from
  SPCR; the allowed RTC physical address is 0x10_1000.
- [ ] Exactly 1,024 pages back the heap at KERNEL_HEAP_ADDR.
- [ ] Comments explain each unsafe operation, address conversion, and locking
  rule. Comments must explain the code you submit; they must not replace it.

Run cargo check from the repository root after each small stage. Do not start
interactive QEMU for this lab unless the instructor asks for it. A successful
source check does not prove that an address map is correct, so also keep the
BSS test from the provided lab material until the BSS path is verified.
