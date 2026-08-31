# Repository guidance

- This repository is a COSC 562 operating-system project. The kernel is in `MyOS/`.
- The kernel is a Rust 2024 `no_std`, `no_main` program for 64-bit RISC-V.
- It boots through Limine in supervisor mode and runs on QEMU `virt` with AIA and ACPI enabled.
- Inspect the repository before you explain or change repository-specific behavior. Prefer small `rg` queries, exclude `MyOS/target/`, and never use `ls -R`.
- Treat `MyOS/src/acpi.rs` as work in progress. Verify statements about the current ACPI implementation against that file and the boot configuration in `MyOS/OPTIONS.sh`.
- In `acpi.rs`, `RsdtRawV1` and `RsdtRawV2` currently describe the RSDP layouts despite their names. The `RSD PTR ` signature identifies the RSDP. Do not describe the RSDP as the RSDT.
- An XSDT entry is one 64-bit physical address. Do not invent a fixed ACPI table address, an entry handle, or x86 APIC details that are not present in this RISC-V repository.
- Run Rust commands from `MyOS/`. Use `cargo check` for a quick source check. Do not start interactive QEMU unless the user asks for it.
- Preserve unrelated working-tree changes and generated build artifacts.
