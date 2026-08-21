/* COSC562 Linker Script For RISC-V 64-bit Systems
 * © Stephen Marz
 * 28-May-2025
 *
 * Permission groups and their ELF flags:
 * Permission numbers are the same as you would use for chmod.
 * Examples:
 *     rwx = 421
 *     If I select 5 = 4 + 1 = r + w
 *   text/trampoline  PT_LOAD  FLAGS(5)  = PF_R | PF_X   (read + execute)
 *   rodata           PT_LOAD  FLAGS(4)  = PF_R          (read only)
 *   data             PT_LOAD  FLAGS(6)  = PF_R | PF_W   (read + write)
 *   bss              PT_NULL  FLAGS(6)  = PF_R | PF_W   (not loaded)
 */

OUTPUT_ARCH("riscv")
ENTRY(_start)

MEMORY
{
    ram (rwx) : ORIGIN = 0xFFFFFFFFA0000000, LENGTH = 128M
}

/* FLAGS values:
 *   PF_X = 1  execute
 *   PF_W = 2  write
 *   PF_R = 4  read
 * Limine checks these flags to determine page table permissions.
 * Without explicit FLAGS the linker may produce incorrect values.
 */
PHDRS
{
    text       PT_LOAD FLAGS(5);   /* R + X — .text                        */
    trampoline PT_LOAD FLAGS(5);   /* R + X — .trampoline (dual-mapped)    */
    rodata     PT_LOAD FLAGS(4);   /* R     — .rodata + Limine requests    */
    data       PT_LOAD FLAGS(6);   /* R + W — .data                        */
    bss        PT_NULL FLAGS(6);   /* R + W — .bss  (not in file)          */
}

SECTIONS
{

    /* -- Read + Execute ------------------------------------------------------
     * .text.init first so the entry point lands at the very beginning of the
     * segment, which is where Limine jumps.
     */
    .text : {
        PROVIDE(__text_start   = .);
        PROVIDE(__kernel_start = .);
        *(.text.init) *(.text .text.*)
        PROVIDE(__text_end     = .);
    } >ram AT>ram :text

    /* -- Trampoline -----------------------------------------------------------
     * Must be page-aligned so a single MMU mapping covers exactly one page.
     * Kept in its own PHDR so the kernel can map it independently in both
     * kernel and user page tables.
     *
     * ALIGN(4096) before: ensures the trampoline starts on a fresh page,
     * giving it a clean boundary from the preceding .text segment.
     * Even though text and trampoline share FLAGS(5), Limine is happy because
     * they are separate PHDRs that each start on a page boundary.
     */
    .trampoline : ALIGN(4096) {
        PROVIDE(__trampoline_start = .);
        *(.trampoline .trampoline.*)
        PROVIDE(__trampoline_end   = .);
        ASSERT(__trampoline_end - __trampoline_start <= 4096,
               "Error: trampoline section exceeds 4 KiB");
        . = ALIGN(4096);             /* pad to a full page                  */
    } >ram AT>ram :trampoline

    /* -- Read Only ------------------------------------------------------------*/
    .rodata : ALIGN(4096){
        PROVIDE(__rodata_start = .);
        *(.rodata .rodata.*)
        PROVIDE(__rodata_end   = .);
    } >ram AT>ram :rodata

    /* -- Limine Protocol ------------------------------------------------------------*/
    /* Must be in a loaded, read-only segment so Limine can find them.
     * KEEP() is mandatory. 
     */
    . = ALIGN(8);
    .limine_requests_start : {
        KEEP(*(.limine_requests_start))
    } >ram AT>ram :rodata

    .limine_requests : {
        KEEP(*(.limine_requests))
    } >ram AT>ram :rodata

    .limine_requests_end : {
        KEEP(*(.limine_requests_end))
    } >ram AT>ram :rodata

    /* -- Autoexec Protocol ---------------------------------------------------*/
    /* Must be in a loaded, read-only segment so we can find them.
     * KEEP() is mandatory. 
     */
    .autoexec ALIGN(8) : {
        PROVIDE(__autoexec_start = .);
        KEEP(*(.autoexec))
        PROVIDE(__autoexec_end = .);
    } >ram AT>ram :rodata
    /* -- Read + Write ---------------------------------------------------------
     * Global pointer must be in the data segment so gp-relative addressing
     * works correctly for both .sdata and .data symbols.
     */
    .data : ALIGN(4096) {
        /* set global pointer 2KiB into the data segment. This is to give the maximum
         * range for gp-relative addressing.
         */
        PROVIDE(__global_pointer$ = . + 0x800);
        PROVIDE(__data_start      = .);
        *(.sdata .sdata.*) *(.data .data.*)
        PROVIDE(__data_end        = .);
    } >ram AT>ram :data

    /* -- BSS ---------------------------------------------------------------
     * PT_NULL means this segment is not present in the ELF file — the kernel
     * zeros it at boot. Must come AFTER .data so it does not interleave with
     * a loaded segment.
     *
     * NOLOAD tells the linker not to include these bytes in the output file,
     * matching PT_NULL semantics.
     */
    .bss (NOLOAD) : ALIGN(4096) {
        PROVIDE(__bss_start = .);
        *(.sbss .sbss.*) *(.bss .bss.*)
        . = ALIGN(8);
        PROVIDE(__bss_end   = .);
    } >ram AT>ram :bss

    PROVIDE(__kernel_end = .);

    /* -- Stack Pointers -----------------------------------------------------*/
    /* Since we are not controlling M-mode, we do not need to set up stack
     * pointers here. Instead, we will use trap_stack field of the scheduled
     * task to manage stack usage in traps.
     */

    /* -- Memory Pool ----------------------------------------------------------
     * Just like the stack, instead of getting the memory pool from the linker
     * script, we will get it from UEFI.
     */

    /* -- Discard --------------------------------------------------------------
     * Exception handling tables are only needed for C++ unwinding.
     * Discarding them reduces image size and avoids spurious segments.
     */
    /DISCARD/ : {
        *(.eh_frame)
        *(.eh_hdr)
        *(.eh*)
        *(.note .note.*)
        *(.comment)
    }
}
