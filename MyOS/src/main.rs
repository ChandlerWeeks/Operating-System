//! # COSC562 Fall 2026 Operating System
//!
//! Operating system entry point: Starts the kernel, and prints LIMINE/ACPI information.
//!
//! Jason Weeks - jweeks12
//! September 13th, 2026
#![no_std]
#![no_main]

use crate::acpi::MadtStructure;
use crate::config::{KERNEL_HEAP_ADDR, NUM_HEAP_PAGES};
use crate::data::{init_kdata, kdata, KernelData};
use crate::mem::{HeapAllocator, PageAllocator};
use crate::print::clear_screen;
use crate::sv48::{MemFlags, PageTableRoot};

const PAGE_SIZE: usize = 4096;

extern crate alloc;

// ============================================================================
// Language Panic Handler
// ============================================================================
#[panic_handler]
fn panic(info: &core::panic::PanicInfo<'_>) -> ! {
    print!("[\x1b[91mOS PANIC\x1b[0m]");
    if let Some(loc) = info.location() {
        print!(" {}@{}.{}", loc.file(), loc.line(), loc.column());
    }
    println!(": {}", info.message());
    // Busy loop to prevent panic!() from returning.
    loop {
        unsafe {
            core::arch::asm!("wfi");
        }
    }
}

// ============================================================================
// Entry Point (_start and main)
// ============================================================================

/// # Entry point
///
/// Coming from the Limine boot loader, we are in S-mode with the MMU turned on.
#[unsafe(no_mangle)]
unsafe extern "C" fn _start() -> ! {
    unsafe extern "C" {
        #[link_name = "__global_pointer$"]
        unsafe static __GLOBAL_POINTER: u8;
    }

    unsafe {
        // Set the global pointer.
        core::arch::asm!(
            "la gp, {address}",
            address = sym __GLOBAL_POINTER,
            options(nostack, nomem),
        );
    }
    main();
    sbi::hart_stop();
}

/// function called by the entry point, used to break into the the kernel. 
fn main() {
    clear_screen();
    initialize_memory();

    // Get the XSDT from the Limine RSDP response.
    let Some(rsdp_vaddr) = limine::rsdp_virt() else {
        debugln!("ACPI: Limine did not provide an RSDP address.");
        sbi::shutdown();
    };
    let Some(xsdt) = acpi::Xsdt::from_rsdp(rsdp_vaddr as usize) else {
        debugln!("ACPI: The RSDP does not contain a valid XSDT address.");
        sbi::shutdown();
    };

    let mut aplic_address = None;
    let mut imsic_address = None;
    let mut ecam_address = None;
    let mut uart_address = None;
    let mut timer_frequency = None;
    let mut imsic_printed = false;

    // Read each ACPI table and save the hardware values.
    for table in xsdt.iter() {
        // Read the RISC-V interrupt controller data.
        if let Some(madt) = table.madt() {
            let mut index = 0;
            while let Some(entry) = madt.get_entry(index) {
                match entry {
                    MadtStructure::Rintc(rintc) => {
                        let address = rintc.imsic_base_address();
                        imsic_address.get_or_insert(address);
                        debugln!(
                            "HART #{} IMSIC @ {:#018X}",
                            rintc.hart_id(),
                            limine::pa_to_va(address as usize)
                        );
                    }
                    MadtStructure::Imsic(imsic) if !imsic_printed => {
                        debugln!(
                            "IMSIC: version {}, supported IDs {}, guest IDs {}, hart index bits {}",
                            imsic.version(), 
                            imsic.num_sup_interrupt_ids(),
                            imsic.num_guest_interrupt_ids(),
                            imsic.hart_index_bits()
                        );
                        imsic_printed = true;
                    }
                    MadtStructure::Aplic(aplic) if aplic_address.is_none() => {
                        let address = aplic.aplic_address();
                        aplic_address = Some(address);
                        debugln!(
                            "APLIC: VA {:#018X}, PA {:#010X}",
                            limine::pa_to_va(address as usize),
                            address
                        );
                    }
                    _ => {}
                }
                index += 1;
            }
        }

        // Read the PCI configuration address.
        if let Some(mcfg) = table.mcfg() {
            ecam_address = mcfg.get_entry(0).map(|entry| entry.base);
        }

        // Read the UART settings.
        if let Some(spcr) = table.spcr() {
            if spcr.address_space_id() == 0 {
                let address = spcr.base_address();
                uart_address = Some(address);
                debugln!(
                    "UART: {:#018X}, freq: {}, irq: {}, irq_type: {}",
                    limine::pa_to_va(address as usize),
                    spcr.uart_frequency(),
                    spcr.irq(),
                    spcr.interrupt_type()
                );
            } else {
                debugln!("ACPI SPCR: UART address is not in system memory.");
            }
        }

        // Read the RISC-V timer frequency.
        if let Some(rhct) = table.rhct() {
            timer_frequency = Some(rhct.time_base_freq());
            debugln!("RHCT: {:?}", rhct);
        }
    }

    // Print the required PCI and timer values.
    print_address("PCI ECAM", ecam_address);
    print_frequency(timer_frequency);

    if aplic_address.is_none() {
        debugln!("ACPI APLIC address: not found");
    }
    if imsic_address.is_none() {
        debugln!("ACPI IMSIC address: not found");
    }
    if uart_address.is_none() {
        debugln!("ACPI UART address: not found");
    } else if let Some(address) = uart_address {
        map_io_page(address as usize);
    }

    // The QEMU virt RTC is at physical address 0x10_1000.
    map_io_page(0x10_1000);

    symbols::print_elf_sections(); 

    sbi::shutdown();
}

/// Build memory services before code uses the Rust heap.
fn initialize_memory() {
    let mut page_allocator = PageAllocator::new();

    for (base, length) in limine::usable_memory_regions() {
        let end = base.checked_add(length).expect("usable memory region overflows");
        let start_vaddr = limine::pa_to_va(base as usize);
        let end_vaddr = limine::pa_to_va(end as usize);

        // Free-list nodes live in pages, so use addresses that the kernel can write.
        unsafe { page_allocator.add_region(start_vaddr, end_vaddr); }
    }

    let mut page_table_root = PageTableRoot::current();
    let heap_flags = MemFlags::new().read().write().accessed().dirty();

    for page_number in 0..NUM_HEAP_PAGES {
        let page = page_allocator
            .alloc_zeroed()
            .expect("out of pages mapping the kernel heap");
        let vaddr = KERNEL_HEAP_ADDR + page_number * PAGE_SIZE;
        let paddr = limine::va_to_pa(page.start_addr());

        page_table_root.map_unmanaged(vaddr, paddr, heap_flags, || {
            page_allocator
                .alloc_zeroed()
                .expect("out of pages for a heap page table")
        });
    }
    riscv::sfence_all();

    let heap_bytes = NUM_HEAP_PAGES * PAGE_SIZE;
    let heap_allocator = unsafe { HeapAllocator::new(KERNEL_HEAP_ADDR, heap_bytes) };
    init_kdata(KernelData::new(page_allocator, heap_allocator, page_table_root));

    map_and_clear_bss();
}

/// Map fresh pages for BSS, then clear its exact byte range.
fn map_and_clear_bss() {
    let start = symbols::Bss::start();
    let size = symbols::Bss::size();
    assert!(start & (PAGE_SIZE - 1) == 0, "BSS start is not page aligned");
    let page_count = size
        .checked_add(PAGE_SIZE - 1)
        .expect("BSS size overflows")
        / PAGE_SIZE;
    let flags = MemFlags::new().read().write().accessed().dirty();

    for page_number in 0..page_count {
        let vaddr = start + page_number * PAGE_SIZE;
        let page = kdata()
            .with_page_allocator(|allocator| allocator.alloc_zeroed())
            .expect("out of pages mapping BSS");
        let paddr = limine::va_to_pa(page.start_addr());

        kdata().with_page_table(|page_table| page_table.map(vaddr, paddr, flags));
    }

    riscv::sfence_all();
    symbols::Bss::as_mut().fill(0);
}

/// Map the page that contains one device physical address.
fn map_io_page(paddr: usize) {
    let page_paddr = paddr & !(PAGE_SIZE - 1);
    let vaddr = limine::pa_to_va(page_paddr);
    let flags = MemFlags::new().read().write().io().accessed().dirty();

    kdata().with_page_table(|page_table| page_table.map(vaddr, page_paddr, flags));
}

/// Prints one detected physical address.
fn print_address(name: &str, address: Option<u64>) {
    match address {
        Some(address) => debugln!("ACPI {} address: {:#018X}", name, address),
        None => debugln!("ACPI {} address: not found", name),
    }
}

/// Prints the detected timer frequency.
fn print_frequency(frequency: Option<u64>) {
    match frequency {
        Some(frequency) => debugln!("ACPI timer frequency: {} Hz", frequency),
        None => debugln!("ACPI timer frequency: not found"),
    }
}

// ============================================================================
// Modules (basically #include for Rust files)
// ============================================================================

pub mod print;
pub mod sbi;
pub mod galloc;
pub mod limine;
pub mod config;
pub mod symbols;
pub mod mem;
pub mod acpi; 
pub mod sync;
pub mod data;
pub mod riscv;
pub mod sv48;
