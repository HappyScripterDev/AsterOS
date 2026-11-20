OUTPUT_FORMAT(elf64-x86-64)
ENTRY(_start)

PHDRS
{
    text   PT_LOAD FLAGS(5);
    rodata PT_LOAD FLAGS(4);
    data   PT_LOAD FLAGS(6);
}

SECTIONS
{
    /* User space currently starts at 4 MiB, below the 2 GiB boundary of the
       small code model. The kernel direct map and image live in the upper
       canonical half, so this range is never aliased. */
    . = 0x0000000000400000;

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
        *(.bss .bss.*)
        *(COMMON)
        . = ALIGN(4096);
    } :data

    /DISCARD/ :
    {
        *(.eh_frame)
        *(.eh_frame_hdr)
        *(.comment)
        *(.note .note.*)
    }
}
