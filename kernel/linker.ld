OUTPUT_FORMAT(elf64-x86-64)
ENTRY(_start)

PHDRS
{
    text   PT_LOAD FLAGS(5); /* R + X */
    rodata PT_LOAD FLAGS(4); /* R     */
    data   PT_LOAD FLAGS(6); /* RW    */
}

SECTIONS
{
    . = 0xFFFFFFFF80000000;
    __kernel_start = .;

    .text : ALIGN(4096)
    {
        KEEP(*(.text.entry))
        *(.text .text.*)
    } :text

    . = ALIGN(4096);
    .rodata : ALIGN(4096)
    {
        *(.rodata .rodata.*)
    } :rodata

    . = ALIGN(4096);
    .data : ALIGN(4096)
    {
        *(.data .data.*)
        *(.got .got.*)
    } :data

    .bss : ALIGN(4096)
    {
        __bss_start = .;
        *(.bss .bss.*)
        *(COMMON)
        . = ALIGN(4096);
        __bss_end = .;
    } :data

    __kernel_end = .;

    /DISCARD/ :
    {
        *(.eh_frame)
        *(.eh_frame_hdr)
        *(.comment)
        *(.note .note.*)
        *(.llvm_addrsig)
    }
}
