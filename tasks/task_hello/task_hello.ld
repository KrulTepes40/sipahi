/* Sipahi SNTM task linker script — task_hello (v1.5-rc1 minimal scaffold).
 *
 * U-23 placeholder addresses — gerçek değerler Sprint U-24 manifest'ten
 * sntm-validate ile generate edilecek. Sprint U-26 kernel loader bu
 * adreslere PMP-protected kopyalayacak.
 */

ENTRY(_start)

MEMORY
{
    TEXT   (rx)  : ORIGIN = 0x80100000, LENGTH = 16K
    RODATA (r)   : ORIGIN = 0x80104000, LENGTH = 4K
    DATA   (rw)  : ORIGIN = 0x80105000, LENGTH = 4K
}

SECTIONS
{
    .text :
    {
        KEEP(*(.text._start))   /* entry point first */
        *(.text*)
    } > TEXT

    .rodata :
    {
        *(.rodata*)
        *(.srodata*)
    } > RODATA

    .data :
    {
        *(.data*)
        *(.sdata*)
    } > DATA

    .bss (NOLOAD) :
    {
        __bss_start = .;
        *(.bss*)
        *(.sbss*)
        *(COMMON)
        __bss_end = .;
    } > DATA

    /* SNTM design v0.5 §4.8.4 — bare-metal'de gereksiz section'lar atılır */
    /DISCARD/ :
    {
        *(.eh_frame)
        *(.eh_frame_hdr)
        *(.got)
        *(.got.plt)
        *(.plt)
        *(.comment)
        *(.note*)
        *(.dynsym)
        *(.dynstr)
        *(.hash)
        *(.gnu.hash)
        *(.init_array)
        *(.fini_array)
        *(.preinit_array)
        *(.ctors)
        *(.dtors)
    }
}

ASSERT(SIZEOF(.text)   <= LENGTH(TEXT),   ".text exceeds TEXT region")
ASSERT(SIZEOF(.rodata) <= LENGTH(RODATA), ".rodata exceeds RODATA region")
ASSERT(SIZEOF(.data)   <= LENGTH(DATA),   ".data exceeds DATA region")
