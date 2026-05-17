/* Sipahi SNTM task linker script — task_world (v1.5 U-27 Phase 5).
 *
 * U-27 FIX-A: task_world.text base = 0x80700000 (task_hello 0x80600000 +
 * 1MB buffer). task_hello stack top = 0x80612000; task_world.text 0x80700000
 * → 0xEE000 (~952KB) margin. RAM_END = 0xA0000000, task_world stack top
 * 0x80712000 → 511MB headroom.
 *
 * sipahi.toml task_world manifest region adresleri + sntm-validate
 * compile-time overlap reject + bu linker script ATOMIC senkron.
 */

ENTRY(_start)

MEMORY
{
    TEXT   (rx)  : ORIGIN = 0x80700000, LENGTH = 16K
    RODATA (r)   : ORIGIN = 0x80704000, LENGTH = 4K
    DATA   (rw)  : ORIGIN = 0x80705000, LENGTH = 4K
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
