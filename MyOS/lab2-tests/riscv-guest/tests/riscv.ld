OUTPUT_ARCH("riscv")
ENTRY(_start)

MEMORY
{
    ram (rwx) : ORIGIN = 0x80200000, LENGTH = 16M
}

SECTIONS
{
    .text : ALIGN(4K) {
        *(.text.init)
        *(.text .text.*)
    } >ram

    .rodata : ALIGN(4K) {
        *(.rodata .rodata.*)
    } >ram

    .data : ALIGN(4K) {
        *(.sdata .sdata.*)
        *(.data .data.*)
    } >ram

    .bss (NOLOAD) : ALIGN(4K) {
        __bss_start = .;
        *(.sbss .sbss.*)
        *(.bss .bss.*)
        *(COMMON)
        __bss_end = .;
    } >ram

    .stack (NOLOAD) : ALIGN(16) {
        __stack_bottom = .;
        . += 64K;
        __stack_top = .;
    } >ram

    /DISCARD/ : {
        *(.eh_frame)
        *(.eh_frame_hdr)
        *(.comment)
    }
}

