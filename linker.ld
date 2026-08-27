OUTPUT_FORMAT("elf64-x86-64")
OUTPUT_ARCH(i386:x86-64)
ENTRY(kernel_main)

PHDRS
{
    text    PT_LOAD FLAGS((1 << 0) | (1 << 2)); /* r-x */
    rodata  PT_LOAD FLAGS((1 << 2));            /* r-- */
    data    PT_LOAD FLAGS((1 << 1) | (1 << 2)); /* rw- */
}

SECTIONS
{
    . = 0xffffffff80000000;

    .text : {
        *(.text .text.*)
    } :text

    . = ALIGN(0x1000);

    /* Limine requests must live in .limine section visibility; keep before rodata */
    . = ALIGN(0x1000);

    /* Limine locates requests by this exact section name. */
    .limine_requests : {
        KEEP(*(.limine_requests_start))
        KEEP(*(.limine_requests))
        KEEP(*(.limine_requests_end))
    } :rodata

    .rodata : {
        *(.rodata .rodata.*)
    } :rodata

    . = ALIGN(0x1000);

    .data : {
        *(.data .data.*)
    } :data

    . = ALIGN(0x1000);

    .bss : {
        *(COMMON)
        *(.bss .bss.*)
    } :data
}
