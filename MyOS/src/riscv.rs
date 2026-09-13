//! # COSC562 Fall 2026 Operating System
//!
//! Operating system entry point: Starts the kernel, and prints LIMINE/ACPI information.
//!
//! Jason Weeks - jweeks12
//! September 13th, 2026
//!
//! RISC-V specific code, including CSR access and other assembly instructions.
//!
//! © Stephen Marz
//! 8 June 2026
use core::arch::asm;
use core::ptr::{read_volatile, write_volatile};

// ==============================================================================
// CSR Macros for read/write/zero/swap
// ==============================================================================
/// `csr_write!(csr, reg)` writes the value of reg to the CSR specified by csr.
#[macro_export]
macro_rules! csr_write
{
    ($csr:expr, $reg:expr) => ({
        let reg = {$reg};
        unsafe {
            core::arch::asm!(concat!("csrw ", $csr, ", {v}\n"), v = in(reg) reg)
        }
    });
}
pub use csr_write;

/// `csr_read!(csr)` reads the value of the CSR specified by csr and returns it as a usize.
#[macro_export]
macro_rules! csr_read
{
    ($csr:expr) => (unsafe {
        let val: usize;
        core::arch::asm!(concat!("csrr {v}, ", $csr, "\n"), v = out(reg) val);
        val
    });
}
pub use csr_read;

/// `csr_zero!(csr)` writes zero to the CSR specified by csr.
#[macro_export]
macro_rules! csr_zero {
    ($csr:expr) => {{ unsafe { core::arch::asm!(concat!("csrw ", $csr, ", zero")) } }};
}
pub use csr_zero;

/// `csr_swap!(csr, reg)` swaps the value of the CSR specified by csr with
/// the value of reg and returns the old value of the CSR.
#[macro_export]
macro_rules! csr_swap
{
    ($csr:expr, $reg:expr) => ({
        let reg = {$reg};
        let ret: usize;
        unsafe {
            core::arch::asm!(concat!("csrrw {o}, ", $csr, ", {v}\n"), o = out(reg) ret, v = in(reg) reg)
        }
        ret
    });
}
pub use csr_swap;

/// `sret!()` invokes the sret instruction to return from a trap to supervisor mode.
/// If it fails, the `panic!()` is called next.
#[macro_export]
macro_rules! sret {
    () => {
        unsafe {
            core::arch::asm!("sret");
            panic!("SRET failed.")
        }
    };
}
pub use sret;

/// `ecall!()` with no arguments invokes the ecall instruction
/// without disturbing the registers.
///
/// `ecall!(x)` with a literal argument x invokes the ecall
/// instruction with x in a7 and does not return a value.
///
/// `ecall!(x)` with a non-literal argument x invokes the ecall instruction
/// with x in a7 and returns the value in a0 as a usize.
#[macro_export]
macro_rules! ecall {

    () => (
        unsafe {
            core::arch::asm!("ecall");
        }
    );
    ($sc:literal) => (
        let val = {$sc};
        unsafe {
            core::arch::asm!("ecall", in("a7") val);
        }
    );
    ($sc:expr) => {
        let val = {$sc};
        unsafe {
            core::arch::asm!("ecall", in("a7") val);
        }
    }
}
pub use ecall;

/// Flush the entire TLB.
pub fn sfence_all() {
    unsafe {
        core::arch::asm!("sfence.vma");
    }
}

/// Flush the TLB entry for the given virtual address.
pub fn sfence_vma(vma: usize) {
    unsafe {
        core::arch::asm!("sfence.vma {v}, zero", v = in(reg) vma);
    }
}

/// Flush the TLB entries for the given ASID.
pub fn sfence_asid(asid: u16) {
    unsafe {
        core::arch::asm!("sfence.vma zero, {a}", a = in(reg) asid);
    }
}

/// Flush the TLB entry for the given virtual address and ASID.
pub fn sfence_vma_and_asid(vma: usize, asid: u16) {
    unsafe {
        core::arch::asm!("sfence.vma {v}, {a}", v = in(reg) vma, a = in(reg) asid);
    }
}

/// `wfi_loop!()` loops with the wfi instruction,
/// which effectively puts the CPU to sleep until
/// an interrupt occurs.
#[macro_export]
macro_rules! wfi_loop {
    () => {
        unsafe {
            loop {
                core::arch::asm!("wfi");
            }
        }
    };
}
pub use wfi_loop;

/// `wfi!()` invokes the wfi instruction which puts the
/// HART to sleep until an interrupt occurs.
#[macro_export]
macro_rules! wfi {
    () => {
        unsafe {
            core::arch::asm!("wfi");
        }
    };
}
pub use wfi;

/// Generic register building macro.
///
/// `buildreg!(x, y, z)`
///
/// will return the bitwise OR of x, y, and z as a usize.
#[macro_export]
macro_rules! buildreg
{
    ($($x: expr), +) => ({
        let mut ret = 0;
        $(
          ret |= {$x};
        )+
        ret
    });
}
pub use buildreg;

/// Build a SATP value from the given ASID and page table address, with MODE=SV32.
///
/// The table address is the full address. This function will convert that address
/// into the physical page number (PPN).
pub const fn build_satp(mode: u64, asid: u16, addr: u64) -> u64 {
    use satp_reg::{ASID_BIT, MODE_BIT, PPN_BIT};
    let asid = (asid as u64) & 0xFFFF;
    let ppn = (addr >> 12) & 0xFFF_FFFF_FFFF;
    buildreg!(mode << MODE_BIT, asid << ASID_BIT, ppn << PPN_BIT)
}

/// Spin in a loop until the value being incremented reaches
/// the given amount, using a volatile variable to prevent optimization.
pub fn spin(amt: u64) {
    let mut v: u64 = 0;
    unsafe {
        while read_volatile(&v) < amt {
            let o = read_volatile(&v);
            write_volatile(&mut v, o + 1);
        }
    }
}

// ==============================================================================
// Hart (Hardware Thread) halt and wait functions. The kernel expects this for
// all machine types.
// ==============================================================================

/// Halt the CPU by entering a low-power state in an infinite loop.
#[inline(always)]
pub fn halt() -> ! {
    loop {
        wait();
    }
}

/// Halt the CPU until it receives an interrupt, then return to the caller. This is useful for low-power waiting.
#[inline(always)]
pub fn wait() {
    unsafe {
        asm!("wfi");
    }
}

/// # Get the current value of the monotonically increasing timer.
///
/// This timer starts at 0 at boot time and increases at 10MHz. It is used
/// to put in the `stimecmp` register to force a timer interrupt for context
/// switches.
pub fn get_ticks() -> u64 {
    let ret: u64;
    unsafe {
        core::arch::asm!("rdtime {value}", value = out(reg) ret);
    }
    ret
}

// ==============================================================================
// [m|s]STATUS register fields
// ==============================================================================
pub mod status_reg {
    pub const SIE_BIT: usize = 1;
    pub const MIE_BIT: usize = 3;
    pub const SPIE_BIT: usize = 5;
    pub const MPIE_BIT: usize = 7;
    pub const SPP_BIT: usize = 8;
    pub const MPP_BIT: usize = 11;
    pub const FS_BIT: usize = 13;

    pub const MPIE: usize = 1 << MPIE_BIT;
    pub const MIE: usize = 1 << MIE_BIT;
    pub const SPIE: usize = 1 << SPIE_BIT;
    pub const SIE: usize = 1 << SIE_BIT;

    pub const MPP_MACHINE: usize = 3 << MPP_BIT;
    pub const MPP_SUPERVISOR: usize = 1 << MPP_BIT;
    pub const MPP_USER: usize = 0 << MPP_BIT;

    pub const SPP_SUPERVISOR: usize = 1 << SPP_BIT;
    pub const SPP_USER: usize = 0 << SPP_BIT;

    pub const FS_OFF: usize = 0 << FS_BIT;
    pub const FS_INITIAL: usize = 1 << FS_BIT;
    pub const FS_DIRTY: usize = 2 << FS_BIT;
    pub const FS_CLEAN: usize = 3 << FS_BIT;
}

// ==============================================================================
// SATP register fields
// ==============================================================================
pub mod satp_reg {
    pub const MODE_SV39: usize = 8;
    pub const MODE_SV48: usize = 9;
    pub const MODE_SV57: usize = 10;
    pub const MODE_BIT: usize = 60;
    pub const MODE_SIZE: usize = 4;

    pub const ASID_BIT: usize = 44;
    pub const ASID_SIZE: usize = 16;

    pub const PPN_BIT: usize = 0;
    pub const PPN_SIZE: usize = 44;
}

// ==============================================================================
// Page Table Entry Fields
// ==============================================================================
pub mod pte {
    // Single-bit permission bits.
    pub const VALID: usize = 1 << 0;
    pub const READ: usize = 1 << 1;
    pub const WRITE: usize = 1 << 2;
    pub const EXECUTE: usize = 1 << 3;
    pub const USER: usize = 1 << 4;
    pub const GLOBAL: usize = 1 << 5;
    pub const ACCESSED: usize = 1 << 6;
    pub const DIRTY: usize = 1 << 7;

    pub const RSW_BIT: usize = 8;
    pub const PPN_0_BIT: usize = 10;
    pub const PPN_1_BIT: usize = 19;
    pub const PPN_2_BIT: usize = 28;
    pub const PPN_3_BIT: usize = 37;
    pub const PPN_4_BIT: usize = 48;
    pub const PBMT_BIT: usize = 61;
    pub const N_BIT: usize = 63;

    // Values for PBMT field (bits 62:61)
    /// # PMA
    /// No memory attributes. Typically we will use none, meaning cacheable.
    pub const PBMT_NONE: usize = 0;
    /// # NC
    /// Non-cacheable, idempotent, weakly-ordered (RVWMO), main memory.
    pub const PBMT_MAIN: usize = 1;
    /// # IO
    /// Non-cacheable, non-idempotent, strongly-ordered (IO ordering), IO.
    pub const PBMT_IO: usize = 2;
}

// ==============================================================================
// [m|s]I[e|p] register fields
// ==============================================================================
pub mod interrupt_reg {
    pub const SSIP_BIT: usize = 1;
    pub const SSIE_BIT: usize = SSIP_BIT;

    pub const MSIP_BIT: usize = 3;
    pub const MSIE_BIT: usize = MSIP_BIT;

    pub const STIP_BIT: usize = 5;
    pub const STIE_BIT: usize = STIP_BIT;

    pub const MTIP_BIT: usize = 7;
    pub const MTIE_BIT: usize = MTIP_BIT;

    pub const SEIP_BIT: usize = 9;
    pub const SEIE_BIT: usize = SEIP_BIT;

    pub const MEIP_BIT: usize = 11;
    pub const MEIE_BIT: usize = MEIP_BIT;

    pub const SSIP: usize = 1 << SSIP_BIT;
    pub const SSIE: usize = 1 << SSIE_BIT;

    pub const MSIP: usize = 1 << MSIP_BIT;
    pub const MSIE: usize = 1 << MSIE_BIT;

    pub const STIP: usize = 1 << STIP_BIT;
    pub const STIE: usize = 1 << STIE_BIT;

    pub const MTIP: usize = 1 << MTIP_BIT;
    pub const MTIE: usize = 1 << MTIE_BIT;

    pub const SEIP: usize = 1 << SEIP_BIT;
    pub const SEIE: usize = 1 << SEIE_BIT;

    pub const MEIP: usize = 1 << MEIP_BIT;
    pub const MEIE: usize = 1 << MEIE_BIT;

    pub const MEDELEG_ALL: usize = 0xb1f7;
}

// ==============================================================================
// Integer register constants
// ==============================================================================
pub const NUM_FREGS: usize = 32;
pub const NUM_XREGS: usize = 32;
pub mod xregs {
    pub const ZERO: usize = 0;
    pub const RA: usize = 1;
    pub const SP: usize = 2;
    pub const GP: usize = 3;
    pub const TP: usize = 4;
    pub const T0: usize = 5;
    pub const T1: usize = 6;
    pub const T2: usize = 7;
    pub const S0: usize = 8;
    pub const S1: usize = 9;
    pub const A0: usize = 10;
    pub const A1: usize = 11;
    pub const A2: usize = 12;
    pub const A3: usize = 13;
    pub const A4: usize = 14;
    pub const A5: usize = 15;
    pub const A6: usize = 16;
    pub const A7: usize = 17;
    pub const S2: usize = 18;
    pub const S3: usize = 19;
    pub const S4: usize = 20;
    pub const S5: usize = 21;
    pub const S6: usize = 22;
    pub const S7: usize = 23;
    pub const S8: usize = 24;
    pub const S9: usize = 25;
    pub const S10: usize = 26;
    pub const S11: usize = 27;
    pub const T3: usize = 28;
    pub const T4: usize = 29;
    pub const T5: usize = 30;
    pub const T6: usize = 31;
}
