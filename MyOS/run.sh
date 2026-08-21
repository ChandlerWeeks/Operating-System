#!/bin/bash

# Run script for QEMU. 
#
# Since there are tons of switches and configuration options
# this script is ran using cargo run.
#
# © Stephen Marz
# 8 June 2026

# Import configuration options.
. "OPTIONS.sh"

###################################################################################
##
## Everything below configures the command line to run QEMU. Unless you know what
## you are doing, you should NOT change anything below.
##
###################################################################################
if [ x"${UEFI_DIR}" != "x" ]; then
    UEFI="-drive if=pflash,format=raw,readonly=on,file=${UEFI_CODE},id=pflash0"
    UEFI+=" -drive if=pflash,format=raw,file=${UEFI_VARS},snapshot=on,id=pflash1"
    UEFI+=" -device pcie-root-port,id=espbus,multifunction=off,slot=1,bus=pcie.0,addr=01.0"
    UEFI+=" -device nvme,serial=beefcafe,bus=espbus,drive=esp0"
    UEFI+=" -drive if=none,format=raw,readonly=off,file=fat:rw:${UEFI_ESP},id=esp0"
fi

if [ x${ENABLE_HDD_CREATE} = "xy" -a x${ENABLE_BLOCK} = "xy" -a ! -e ${HDD} ]; then
    RESULT=$(qemu-img create -f raw ${HDD} ${DISK_SIZE} 2>&1 > /dev/null)
    if [ $? -ne 0 ]; then
        echo "[${HDD}]: could not create: ${RESULT}"
        exit 3
    fi
fi

PARAMS+=" -machine ${MACH}"
PARAMS+=" -cpu ${CPU}"
PARAMS+=" -d guest_errors,unimp"
PARAMS+=" -smp ${NUMCPUS}"
PARAMS+=" -m ${MEM}"
if [ x${VNC} != "x" ]; then
    PARAMS+=" -vnc ${VNC}"
fi
if [ x${DEBUGSOCK} != "x" ]; then
    PARAMS+=" -gdb ${DEBUGSOCK}"
fi
if [ x${MONITOR} != "x" ]; then
    PARAMS+=" -serial stdio -monitor ${MONITOR}"
else
    PARAMS+=" -serial mon:stdio"
fi

# PARAMS+=" -serial mon:stdio -serial tcp:localhost:4321,server=on,wait=off"
# REAL TIME CLOCK
PARAMS+=" -rtc base=localtime,clock=host"

# MEMORY
# PARAMS+=" -object memory-backend-file,id=mem0,mem-path=nvdimm.img,size=32M"
# PARAMS+=" -device nvdimm,id=nvdimm0,memdev=mem0,addr=0xB0000000,slot=0"

# UEFI
if [ x"${UEFI}" != "x" ]; then
    PARAMS+=" ${UEFI}"
fi

# PCI
if [ x${ENABLE_PCI} = "xy" ]; then
    PARAMS+=" -device pcie-root-port,id=bridge1,multifunction=off,slot=2,bus=pcie.0,addr=02.0"
    PARAMS+=" -device pcie-root-port,id=bridge2,multifunction=off,slot=3,bus=pcie.0,addr=03.0"
    PARAMS+=" -device pcie-root-port,id=bridge3,multifunction=off,chassis=2,slot=4,bus=pcie.0,addr=04.0"
    PARAMS+=" -device pcie-root-port,id=bridge4,multifunction=off,chassis=3,slot=5,bus=pcie.0,addr=05.0"

    if [ x${ENABLE_BLOCK} = "xy" ]; then
        # BRIDGE 1 (Block Devices)
        # PARAMS+=" -device vhost-vsock-pci-non-transitional,bus=bridge2,guest-cid=9,id=vsock"
        PARAMS+=" -device virtio-blk-pci-non-transitional,drive=hdd0,bus=bridge1,id=virtio.blk0"
        PARAMS+=" -drive if=none,format=raw,file=fat:rw:${HDD},id=hdd0"
    fi

    if [ x${ENABLE_ENTROPY} = "xy" ]; then
        # BRIDGE 2 (Entropy Devices)
        PARAMS+=" -device virtio-rng-pci-non-transitional,bus=bridge2,id=virtio.rng0"
    fi

    if [ x${ENABLE_PMEM} = "xy" ]; then
        PARAMS+=" -object memory-backend-file,id=mem0,mem-path=pmem.img,size=8M,share=on"
        PARAMS+=" -device virtio-pmem-pci,id=pmem0,memdev=mem0,bus=bridge2"
    fi

    if [ x${ENABLE_CONSOLE} = "xy" ]; then
        # BRIDGE 3 (Virtual Console Devices)
        if [ x${ATTACH_VSTDIO} = "xy" ]; then
            PARAMS+=" -chardev stdio,id=char0"
        else
            PARAMS+=" -chardev socket,id=char0,path=${VCONSOLE_SOCKNAME},server=on,wait=off"
        fi
        PARAMS+=" -device virtio-serial-pci,bus=bridge3,id=virtio.serial0"
        PARAMS+=" -device virtserialport,chardev=char0,name=virtio.console0"
    fi

    if [ x${ENABLE_NET} = "xy" ]; then
        # BRIDGE 4 (Network Devices)
        PARAMS+=" -device virtio-net-pci-non-transitional,netdev=net0,bus=bridge4,id=virtio.net0,romfile= "
        if [ x${ATTACH_VSOCK_TCP} = "xy" ]; then
            PARAMS+=" -netdev user,id=net0,hostfwd=tcp::35555-:22"
        else
            PARAMS+=" -netdev stream,id=net0,server=on,addr.type=unix,addr.path=${NETDEV_SOCKNAME}"
        fi
    fi

    # Non-volatile Memory Express (NVMe) devices
    if [ x${ENABLE_NVME_BLOCK} = "xy" ]; then
        PARAMS+=" -device pcie-root-port,id=nvmebus,multifunction=off,slot=6,bus=pcie.0,addr=06.0"
        PARAMS+=" -device nvme,serial=deadbeef,bus=nvmebus,drive=nvme0,id=pci.nvme0"
        PARAMS+=" -drive if=none,format=raw,file=${NVME},id=nvme0"
    fi
fi

# GRAPHICS
if [ x"${GRAPHICS}" = "xy" -a x"${ENABLE_PCI}" = "xy" ]; then
    PARAMS+=" -display ${DISPLAY_BACKEND}"
    # Add GPU, keyboard, and tablet for graphics mode
    PARAMS+=" -device virtio-gpu-pci,bus=pcie.0,id=virtio.gpu0"
    PARAMS+=" -device virtio-keyboard-pci,bus=pcie.0,id=virtio.keyboard0"
    PARAMS+=" -device virtio-tablet-pci,bus=pcie.0,id=virtio.tablet0"
else
    PARAMS+=" -display none"
    PARAMS+=" -nographic"
fi

T=""
if [ x"$DO_TRACES" = "xy" ]; then
    for t in $TRACES; do
        T+="--trace $t "
    done
fi

if [ x${ECHO} = "xy" ]; then
    echo qemu-system-${ARCH} \
    ${ARCH_ARGS} \
    ${PARAMS} \
    $T
fi
if [ x${DISABLE} != "xy" ]; then
    exec qemu-system-${ARCH} \
    ${ARCH_ARGS} \
    ${PARAMS} \
    $T
fi

