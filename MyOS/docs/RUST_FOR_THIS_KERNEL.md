# Rust for This RISC-V Kernel

This is a short working guide for this repository. It starts with normal Rust
and ends at the unsafe code that a small operating system needs. Read the
sections in order the first time.

## 1. What this project is

This crate builds a 64-bit RISC-V kernel for QEMU. It uses the Limine boot
protocol and runs in RISC-V supervisor mode.

It does not use an operating system or the Rust standard library. Therefore it
uses `#![no_std]` and `#![no_main]`. The kernel must supply services that a
normal program gets from an OS, such as startup, panic handling, output,
memory allocation, and locking.

The active crate root is `src/main.rs`. It currently includes these modules:

| File | Purpose |
| --- | --- |
| `src/main.rs` | Kernel entry point, panic handler, first kernel code. |
| `src/print.rs` | SBI-backed text and debug output macros. |
| `src/sbi.rs` | RISC-V Supervisor Binary Interface (SBI) calls. |
| `src/galloc.rs` | Global allocator placeholder. It returns null now. |

The repository also has planned code that is **not yet included** by
`main.rs`:

| File | Planned purpose |
| --- | --- |
| `src/riscv.rs` | CSR access, TLB flushes, register bits, and RISC-V macros. |
| `src/sync.rs` | IRQ-safe spin mutex and one-time initialization. |
| `src/acpi.rs` | ACPI table data and parsing. It is incomplete. |
| `src/limine.rs` | Limine request data. It is incomplete. |

Do not add an incomplete module to `main.rs` only to “turn it on.” The compiler
will then compile that file too.

`.cargo/config.toml` selects `riscv64gc-unknown-none-elf`, uses
`riscv64gc-virt.ld`, and starts QEMU through `run.sh`.

## 2. Build and run loop

Use the project configuration. Do not use host defaults by accident.

```sh
cargo check
cargo build
cargo run
```

`cargo check` is the fast type and borrow check. `cargo build` also links the
kernel ELF. `cargo run` uses `run.sh` and starts QEMU.

Use format before you save a Rust change:

```sh
cargo fmt --check
cargo fmt
```

Build warnings are useful. In kernel code, treat a warning as a question that
needs an answer.

## 3. Start with values and types

Rust makes the size and signedness of values clear. This matters for registers,
addresses, bytes, and hardware data.

```rust
let hart_id: u64 = 0;
let byte: u8 = b'A';
let word: usize = 0x8000_0000;
let is_ready: bool = true;
```

Use these types for this kernel:

- `u8` for a byte, device data, and text bytes.
- `u16`, `u32`, and `u64` for fields whose hardware format states that size.
- `usize` for an address, a pointer-sized register value, an index, or a byte
  count in the current target.
- `bool` for a true or false state.

An integer cast can lose data. State why a cast is safe when the value comes
from hardware or a pointer.

```rust
let page_number = physical_address >> 12;
let hart_id: usize = raw_hart_id as usize; // Safe only after a range check.
```

Use `const` for a named value with no storage. Use `static` for one stored value
for the whole kernel.

```rust
const PAGE_SIZE: usize = 4096;
static BOOTED: AtomicBool = AtomicBool::new(false);
```

## 4. Functions, control flow, and never-returning code

A function has typed inputs and a typed result. The final expression is the
result when there is no semicolon.

```rust
fn page_base(address: usize) -> usize {
    address & !(PAGE_SIZE - 1)
}
```

Use `if` for a choice and `match` when you must handle every case. `match` is a
good fit for SBI error codes and trap causes.

```rust
match error {
    SbiError::Success => { /* continue */ }
    other => panic!("SBI error: {other}"),
}
```

`-> !` means that a function never returns. This is correct for `_start`,
`panic`, a halt loop, and successful power-off code.

```rust
fn halt() -> ! {
    loop {
        unsafe { core::arch::asm!("wfi") }
    }
}
```

## 5. Ownership, references, and lifetimes

Ownership is Rust’s main protection against use-after-free and data races.
Every value has one owner. Moving a value gives its ownership to a new place.

```rust
let frame = WakeFrame::default();
start_hart(frame);       // `frame` moves into the function.
// use(frame);           // Error: it was moved.
```

Use a shared reference, `&T`, when code only reads. Use a mutable reference,
`&mut T`, when code changes a value. At one time, Rust permits either many
shared references or one mutable reference. This rule is very useful for page
tables and device state.

```rust
fn enable(entry: &mut usize) {
    *entry |= 1;
}
```

A lifetime says how long a reference is valid. You will often let the compiler
infer it. Write one when a returned reference comes from an input.

```rust
fn first_byte(bytes: &[u8]) -> Option<&u8> {
    bytes.first()
}
```

Do not create a normal reference from an address that may be invalid, unmapped,
misaligned, or changed by hardware. Use a raw pointer at that boundary first.

## 6. Structs, enums, and layout

Use a `struct` to name related data. Use an `enum` when a value can be one of a
known set of states.

```rust
struct Page {
    physical_address: usize,
    flags: usize,
}

enum MapError {
    OutOfMemory,
    AlreadyMapped,
}
```

Rust may arrange normal struct fields in its own way. Use `#[repr(C)]` when
firmware, assembly, or a binary format reads the structure. The project uses it
for `WakeFrame`.

```rust
#[repr(C)]
struct WakeFrame {
    sepc: usize,
    sstatus: usize,
}
```

`#[repr(C, packed)]` removes padding. Use it only for a packed external data
format, such as an ACPI table. A field in a packed struct can be unaligned. Do
not take a normal reference to such a field. Copy it with an unaligned read or
parse bytes instead.

## 7. Traits: small contracts

A trait states what an item can do. `core` provides traits even when `std` is
not present.

This project implements `core::fmt::Write`. That lets `write!` send formatted
text through `KernelOut` to `sbi::debug_write_char`.

```rust
impl core::fmt::Write for KernelOut {
    fn write_str(&mut self, text: &str) -> core::fmt::Result {
        for byte in text.bytes() {
            crate::sbi::debug_write_char(byte);
        }
        Ok(())
    }
}
```

`From` is another useful trait. `impl From<i64> for SbiError` changes a raw SBI
status into a named error. This makes call sites clearer.

Prefer the standard `Result<T, E>` for new code unless this project has a clear
reason to keep its local `SbiResult<T>`. Avoid `unwrap()` in code that can fail
at run time. Handle the error or use a deliberate kernel panic.

## 8. `no_std`, `core`, and memory

`std` needs an operating system. A kernel does not have one below it, so use
`core` for basic types, formatting, atomics, and traits.

`alloc` is optional. It provides `Box`, `Vec`, `String`, and similar types, but
it needs a working global allocator. In this repository, `MyGlobalAllocator`
currently returns `null_mut()`. Therefore do not use heap-backed types until a
real allocator is complete and initialized.

Early kernel code should prefer fixed storage:

```rust
static mut EARLY_PAGES: [u8; 16 * 4096] = [0; 16 * 4096];
```

Do not access `static mut` directly in normal code. It is shared mutable state
and needs an `unsafe` block. A safer design is an `UnsafeCell` inside a lock, or
an allocator that owns the storage.

The `#[panic_handler]` in `main.rs` is required because `std` does not provide
one. Keep it small, avoid allocation, print only through known-good output, and
then halt.

## 9. Modules and visibility

`mod name;` compiles `src/name.rs` and makes it a child module. `pub` exposes an
item outside its module. Use a narrow public interface. Keep register details
private behind a small safe function when possible.

```rust
pub mod sbi;

pub fn poweroff() -> ! { /* SBI call */ }
```

Within the crate, `crate::sbi::poweroff()` gives an exact path. This is useful in
macros because the call site can be in a different module.

## 10. Safe wrappers around unsafe boundaries

`unsafe` is not “turn off safety.” It marks a claim that the programmer must
prove. Keep the unsafe block as small as possible and put the proof in a
`SAFETY:` comment beside it.

These are unsafe kernel boundaries:

- Inline assembly and SBI `ecall` instructions.
- Raw pointer reads and writes.
- Memory-mapped I/O.
- A custom allocator.
- Shared mutable data through `UnsafeCell`.
- Manual `Send` and `Sync` implementations.

Make the outer API safe only when it can enforce all rules. Example:

```rust
/// Read one 32-bit register at a valid MMIO address.
pub fn read_register(address: *const u32) -> u32 {
    // SAFETY: The driver owns this valid, aligned MMIO register address.
    unsafe { core::ptr::read_volatile(address) }
}
```

Use `read_volatile` and `write_volatile` for device registers. A normal read or
write can be removed or moved by the compiler. Volatile access does not replace
locking, cache control, or CPU memory fences.

## 11. FFI, the entry point, and inline assembly

The boot loader does not call Rust `main`. It jumps to `_start`. The entry point
in `main.rs` uses a C ABI and a stable exported symbol:

```rust
#[unsafe(no_mangle)]
unsafe extern "C" fn _start() -> ! {
    // Set `gp`, then call normal Rust code.
    main();
    sbi::hart_stop();
}
```

Rust 2024 requires unsafe attributes to say that their effect is deliberate.
The linker script must use the same `_start` name.

Use `core::arch::asm!` only for instructions that Rust cannot express. State
inputs, outputs, clobbers, and options exactly. A wrong assembly contract can
make safe Rust fail.

```rust
fn wait_for_interrupt() {
    // SAFETY: `wfi` is valid in the current supervisor-mode kernel context.
    unsafe { core::arch::asm!("wfi", options(nomem, nostack)) }
}
```

`options(nomem, nostack)` is a claim. Use it only if the instruction does not
read or write memory and does not change the stack. Do not copy these options to
a different instruction without checking its behavior.

## 12. Macros: code generators, not functions

A macro call ends in `!`: `print!`, `println!`, `debugln!`, and `sbicall!` are
project examples. A declarative macro uses `macro_rules!` and expands into Rust
syntax before the compiler type-checks it. It is not C-style text replacement.

The main reasons to use one here are a repeated instruction pattern, a format
interface with variable arguments, or compile-time call-site data.

```rust
#[macro_export]
macro_rules! set_bits {
    ($value:expr, $($bit:expr),+ $(,)?) => {{
        let mut out = $value;
        $(out |= $bit;)+
        out
    }};
}
```

`$($bit:expr),+` means “one or more expressions, separated by commas.” The
repeated `$(out |= $bit;)+` uses each captured expression.

For exported macros, use `$crate::module::item`, as `print!` does. It names the
defining crate even when another module calls the macro. Put side effects in a
block and evaluate each input once with `let` when needed. This avoids surprising
behavior such as running a device read twice.

Read the Rust By Example `macro_rules!` page when you add or change a macro:
<https://doc.rust-lang.org/rust-by-example/macros.html>. Use a function instead
when a function is enough. Functions give clearer types, errors, and debugging.

## 13. Interrupt-safe shared state and SMP

The QEMU configuration starts four harts. Shared kernel data must work when one
hart or an interrupt handler accesses it at the same time as another.

Use atomics for small independent state. `AtomicBool` is good for a lock flag.
Use a mutex for data that needs a group of reads and writes to act as one unit.

```rust
static TICKS: AtomicUsize = AtomicUsize::new(0);

fn timer_interrupt() {
    TICKS.fetch_add(1, Ordering::Relaxed);
}
```

The planned `sync.rs` shows the next level: `Mutex<T>`, `MutexGuard`,
`CriticalSection`, and `OnceLock<T>`.

- `UnsafeCell<T>` is the legal base for mutable data behind a shared reference.
- `AtomicBool::compare_exchange` changes a lock from false to true as one atomic
  action.
- `Ordering::Acquire` on lock and `Ordering::Release` on unlock make protected
  writes visible to the next lock holder.
- `Drop` releases a `MutexGuard` at end of scope. This is RAII.
- `CriticalSection` saves and disables interrupts on creation. Its `Drop` code
  restores the old interrupt state.

Keep the guard in a small scope:

```rust
{
    let mut state = DEVICE_STATE.lock_irqsave();
    state.enabled = true;
} // First unlock. Then restore the prior interrupt state.
```

Do not lock the same non-reentrant spin mutex twice on one hart. Do not call a
function that may need the same lock while you hold it. An interrupt handler can
also deadlock on a lock held by interrupted code. Use the IRQ-save lock form
when that is a possible path.

## 14. A safe order for new kernel work

Build features in this order:

1. Keep the boot and SBI text path working.
2. Add typed constants and safe wrappers for one hardware feature.
3. Add a small unit of raw-pointer or assembly code with a clear safety rule.
4. Add tests that do not need hardware, then test in QEMU.
5. Add locking before shared state reaches more than one hart.
6. Add allocation only after the allocator has real storage and initialization.
7. Add ACPI and Limine parsing only after bounds, alignment, and checksum rules
   are defined.

Before each unsafe change, write answers to these questions:

- Who owns this memory or register?
- Is the address valid, mapped, aligned, and the correct width?
- Can an interrupt or another hart access it now?
- What must happen before and after this instruction?
- Can a safe caller break the rule? If yes, make the API `unsafe` or redesign it.

## 15. Practical rules for this codebase

- Prefer `core` over `std`.
- Prefer a type, enum, or small function over raw numbers and repeated code.
- Keep hardware code behind a narrow module API.
- Make every unsafe block local and explain its safety rule.
- Use `#[repr(C)]` for an ABI or binary-layout promise; do not add it by habit.
- Do not use the heap until `galloc.rs` is real.
- Do not enable `acpi`, `limine`, `riscv`, or `sync` in `main.rs` until each file
  compiles and has been reviewed.
- Let scopes drop lock guards. Do not hand-unlock a safe guard.
- Run `cargo fmt`, then `cargo check`, for each small change.

Rust will not make kernel work risk-free. It does make the risk visible. Keep
unsafe code small, state its rules, and expose safe operations to the rest of
the kernel.
