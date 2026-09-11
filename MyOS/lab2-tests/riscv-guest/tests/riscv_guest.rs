#![no_std]
#![no_main]

use core::arch::{asm, global_asm};
use core::panic::PanicInfo;

global_asm!(
    r#"
    .section .text.init
    .globl _start
_start:
    la sp, __stack_top
    call rust_main
1:
    wfi
    j 1b
"#
);

const SBI_EXT_BASE: usize = 0x10;
const SBI_EXT_DEBUG_CONSOLE: usize = 0x4442_434e;
const SBI_EXT_SYSTEM_RESET: usize = 0x5352_5354;
const SBI_BASE_GET_SPEC_VERSION: usize = 0;
const SBI_DEBUG_CONSOLE_WRITE_BYTE: usize = 2;
const SBI_SYSTEM_RESET: usize = 0;
const SBI_RESET_TYPE_SHUTDOWN: usize = 0;
const SBI_RESET_REASON_NONE: usize = 0;
const SBI_RESET_REASON_FAILURE: usize = 1;

#[inline]
fn console_byte(byte: u8) {
    let _ = sbi_call(
        SBI_EXT_DEBUG_CONSOLE,
        SBI_DEBUG_CONSOLE_WRITE_BYTE,
        byte as usize,
        0,
    );
}

fn console(text: &str) {
    for byte in text.bytes() {
        console_byte(byte);
    }
}

#[inline]
fn sbi_call(extension: usize, function: usize, arg0: usize, arg1: usize) -> (isize, usize) {
    let error: isize;
    let value: usize;
    unsafe {
        asm!(
            "ecall",
            inlateout("a0") arg0 => error,
            inlateout("a1") arg1 => value,
            in("a6") function,
            in("a7") extension,
            options(nostack)
        );
    }
    (error, value)
}

fn shutdown(reason: usize) -> ! {
    let _ = sbi_call(
        SBI_EXT_SYSTEM_RESET,
        SBI_SYSTEM_RESET,
        SBI_RESET_TYPE_SHUTDOWN,
        reason,
    );
    loop {
        unsafe { asm!("wfi", options(nomem, nostack)) };
    }
}

fn fail(message: &str) -> ! {
    console("LAB2_RISCV_TESTS_FAIL: ");
    console(message);
    console("\n");
    shutdown(SBI_RESET_REASON_FAILURE)
}

#[unsafe(no_mangle)]
extern "C" fn rust_main() -> ! {
    if core::mem::size_of::<usize>() != 8 {
        fail("usize is not 64 bits");
    }

    let sstatus: usize;
    unsafe {
        asm!("csrr {}, sstatus", out(reg) sstatus, options(nomem, nostack));
    }
    core::hint::black_box(sstatus);

    let (error, specification_version) = sbi_call(SBI_EXT_BASE, SBI_BASE_GET_SPEC_VERSION, 0, 0);
    if error != 0 || specification_version == 0 {
        fail("SBI base extension is not available");
    }

    console("LAB2_RISCV_TESTS_PASS\n");
    shutdown(SBI_RESET_REASON_NONE)
}

#[panic_handler]
fn panic(_info: &PanicInfo<'_>) -> ! {
    fail("guest panic")
}
