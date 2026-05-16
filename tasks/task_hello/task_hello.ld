/* Sipahi SNTM task linker script — task_hello (v1.5 U-26 Phase 4).
 *
 * U-26 FIX-A: NATIVE_TASK_BASE = 0x80600000 (eski 0x80100000 wasm_arena
 * ortasında çakışıyordu). sipahi.toml task_hello manifest region adresleri
 * + sipahi.ld __native_task_base sabit + bu linker script ATOMIC senkron.
 *
 * Kernel loader (load_task_hello) sntm-pack output bin'lerini bu adreslere
 * PMP-protected NAPOT region'lara bounded_copy + zero_fill ile yerleştirir.
 */

ENTRY(_start)

MEMORY
{
    TEXT   (rx)  : ORIGIN = 0x80600000, LENGTH = 16K
    RODATA (r)   : ORIGIN = 0x80604000, LENGTH = 4K
    DATA   (rw)  : ORIGIN = 0x80605000, LENGTH = 4K
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
