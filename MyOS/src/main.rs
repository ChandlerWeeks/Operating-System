//! # COSC562 Fall 2026 Operating System
//!
//! Operating system entry point: Starts the kernel, and prints LIMINE/ACPI information.
//!
//! Jason Weeks - jweeks12
//! September 13th, 2026
#![no_std]
#![no_main]

use crate::acpi::MadtStructure;
use crate::print::clear_screen;

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

fn main() {
    // Print the ACPI hardware values before the system shuts down.
    clear_screen();

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

    for table in xsdt.iter() {
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

        if let Some(mcfg) = table.mcfg() {
            ecam_address = mcfg.get_entry(0).map(|entry| entry.base);
        }

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

        if let Some(rhct) = table.rhct() {
            timer_frequency = Some(rhct.time_base_freq());
            debugln!("RHCT: {:?}", rhct);
        }
    }

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
    }

    sbi::shutdown();
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
pub mod acpi; 
