# Lab 2 acceptance tests

Run the suite from its package directory:

```sh
cd lab2-tests
cargo test
```

The kernel uses the bare-metal `riscv64gc-unknown-none-elf` target. That target
does not provide Rust's normal test harness or a repository file system. The
top-level test process runs the source-contract checks on the development host.
It also builds a separate `no_std` test image for RV64 and boots that image with
`qemu-system-riscv64`. Both parts must pass.

The RISC-V guest uses the same important machine settings as `OPTIONS.sh`:

- QEMU `virt`;
- APLIC and IMSIC;
- ACPI enabled;
- SV48 enabled;
- four harts; and
- the repository's OpenSBI image.

`qemu-system-riscv64` must be available in `PATH`.

The tests come from the COSC 562 “Limine, ACPI, and Config Lab” specification.
They check the following items:

- the required source modules and configuration constants;
- the Limine protocol IDs, request sections, request parameters, and response
  reader contracts;
- virtual-address and physical-address conversion rules;
- the ACPI RSDP, XSDT, MADT, MCFG, SPCR, and RHCT data contracts;
- the required QEMU, Limine, and linker configuration;
- the required initialization output in `main.rs`; and
- a warning-free RISC-V kernel source check; and
- a bare-metal RV64 and SBI smoke test in QEMU.

The suite does not check comments or documentation.
