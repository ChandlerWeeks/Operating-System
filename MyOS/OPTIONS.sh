#!/bin/bash

# Options imported by the run.sh script.
#
# © Stephen Marz
# 8 June 2026

# Simple configuration options.
#
# * DISABLE does not invoke the qemu command at the end.
# * ECHO outputs the QEMU command that will run.
DISABLE="n"
ECHO="n"
# Traces give extra information about what QEMU is doing. This can be used for debugging,
# but it can also be very verbose. If you want to enable traces, set DO_TRACES="y" and
# specify which traces you want to see in the TRACES variable. Add multiple traces by
# separating by a space. For example, TRACES="virtio* riscv_aplic_*". 
#
# You can find a list of available traces in the QEMU documentation.
DO_TRACES="n"
TRACES="riscv_aplic_*"
# Architecture/machine configuration
#
# * riscv64 is 64-bit RISC-V.
# * BIOS is automatically loaded at 0x8000_0000.
# * Machine is QEMU-virt, with IMSIC enabled, and ACPI turned on.
# * CPU is RV64 with different extensions turned on and off.
ARCH="riscv64"  # "x86_64", "i386", "riscv32", or "riscv64"
ARCH_ARGS="-bios sbi.bin"   # "-bios none"
MACH="virt,aia=aplic-imsic,acpi=on"
CPU="rv64,svpbmt=on,sstc=on,svadu=on,sv48=on,h=off,v=off,pmp=off,zihpm=off,zicntr=off"  # "rv32" or "rv64" or "qemu64"
# QEMU Options.
#
# * GRAPHICS="n" may speed up boot times.
# * With GRAPHIC="y", connect to vnc or curses.
# * MEM sets the amount of RAM starting at 0x8000_0000
# * NUMCPUS sets the number of HARTS.
GRAPHICS="y"
DISPLAY_BACKEND="vnc=::1:1"  # or "curses"
MEM="128M"
NUMCPUS="4"
DISK_SIZE="32M"
HDD="local"
NVME="fat:rw:local/nvme"

# Machine Options
ENABLE_PCI="y"
ENABLE_BLOCK="y"
ENABLE_NVME_BLOCK="y"
ENABLE_ENTROPY="y"
ENABLE_NET="n"
ENABLE_CONSOLE="n"
ENABLE_PMEM="n"
ENABLE_HDD_CREATE="n"   # Create HDD if it doesn't exist
ATTACH_VSTDIO="n"
ATTACH_VSOCK_TCP="n"

# How to setup the monitor. 
#
# If MONITOR is commented out, it will use a multiplexed monitor.
# A multiplexed monitor can be switched back and forth using CTRL-a, 
# followed by c.
#
# UNIX sockets can be connected to using `socat`:
#    socat UNIX-CONNECT:./monitor.sock STDIO
# MONITOR="unix:path=monitor.sock,server,nowait"
# Telnet can be connected to using telnet, socat, or nc
#     socat TCP-CONNECT:localhost:1234 STDIO
# MONITOR="telnet:localhost:1234,server,nowait"

# Only if ENABLE_CONSOLE="y" and ATTACH_VSTDIO="n"
VCONSOLE_SOCKNAME="vconsole.sock"
# Only if ATTACH_VSOCK_TCP="n"
NETDEV_SOCKNAME="net.sock"

# Debugging socket. Can be a TCP or UNIX socket. To prevent
# creation of a debugging socket, comment out the DEBUGSOCK line.
DEBUGSOCK="unix:debug.sock,server,nowait"
# Unified Extensible Firmware Interface
#
# The UEFI_DIR is the root where the UEFI images and ESP are
# located.
UEFI_DIR="uefi"
UEFI_CODE="${UEFI_DIR}/cosc562-riscv-code.fd"
UEFI_VARS="${UEFI_DIR}/cosc562-riscv-vars.fd"
UEFI_ESP="${UEFI_DIR}/esp/"
