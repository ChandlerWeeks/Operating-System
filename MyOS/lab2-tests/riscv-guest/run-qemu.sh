#!/bin/sh
set -eu

test_binary="$1"
qemu_binary="$(command -v qemu-system-riscv64 || true)"

if [ -z "$qemu_binary" ]; then
    echo "qemu-system-riscv64 is required for the Lab 2 RISC-V tests" >&2
    exit 127
fi

output_file="$(mktemp -t myos-lab2-riscv.XXXXXX)"
trap 'rm -f "$output_file"' EXIT HUP INT TERM

set +e
"$qemu_binary" \
    -machine virt,aia=aplic-imsic,acpi=on \
    -cpu rv64,svpbmt=on,sstc=on,svadu=on,sv48=on,h=off,v=off,pmp=off,zihpm=off,zicntr=off \
    -smp 4 \
    -m 128M \
    -bios ../../sbi.bin \
    -kernel "$test_binary" \
    -display none \
    -serial stdio \
    -monitor none \
    -no-reboot \
    >"$output_file" 2>&1
test_status="$?"
set -e

sed -n '1,240p' "$output_file"

if [ "$test_status" -ne 0 ]; then
    exit "$test_status"
fi

if ! grep -q "LAB2_RISCV_TESTS_PASS" "$output_file"; then
    echo "the RISC-V guest did not report a passing test result" >&2
    exit 1
fi
