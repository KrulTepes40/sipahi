/* sipahi.ld — Sipahi Microkernel Linker Script */
/* QEMU virt machine, RV64, RAM @ 0x80000000   */

ENTRY(_start)

MEMORY {
    /* 4MB WASM arena + kernel + stack. QEMU virt = 512MB, 8MB güvenli. */
    RAM (rwx) : ORIGIN = 0x80000000, LENGTH = 8M
}

SECTIONS {
    /* R0: Kernel .text (32KB bütçe) */
    .text : ALIGN(4) {
        __text_start = .;
        *(.text.boot)       /* _start en başta olmalı */
        *(.text .text.*)
        __text_end = .;
    } > RAM

    /* R2: Kernel .rodata (8KB bütçe) */
    .rodata : ALIGN(4) {
        __rodata_start = .;
        *(.rodata .rodata.*)
        __rodata_end = .;
    } > RAM

    /* R1: Kernel .data (16KB bütçe) */
    .data : ALIGN(4) {
        __data_start = .;
        *(.data .data.*)
        __data_end = .;
    } > RAM

    /* BSS — sıfırlanacak veriler */
    /* KRİTİK: boot.S'de __bss_start → __bss_end arası sıfırlanmalı!   */
    /* ALIGN(8): boot.S `sd` (8-byte store) + addi a0,a0,8 ile temizler */
    /* __bss_end de ALIGN(8): son bloğun taşmasını önler                 */
    .bss : ALIGN(8) {
        __bss_start = .;
        *(.bss .bss.*)
        *(COMMON)
        . = ALIGN(8);
        __bss_end = .;
    } > RAM

    /* Kernel stack (16KB) — Sprint 13: Ed25519/BLAKE3 test frame'leri için büyütüldü */
    /* config.rs::KERNEL_STACK_SIZE ile senkron tutulmalı                               */
    . = ALIGN(16);
    __stack_bottom = .;
    . += 16384;
    __stack_top = .;

    /* Stack guard — PMP ile korunacak */
    __stack_guard = __stack_bottom;

    /* _end: Tüm kernel belleğinin (data+bss+stack) bittiği nokta.     */
    /* PMP RW bölgesinin TOR üst sınırı bu sembolü kullanmalıdır.       */
    _end = .;

    /* ═══════════════════════════════════════════════════════════ */
    /* Sprint'lerde eklenecek bölgeler:                           */
    /*                                                             */
    /* Sprint 8:  IPC pool (8KB)                                  */
    /*   __ipc_pool_start / __ipc_pool_end                        */
    /*                                                             */
    /* Sprint 11: Blackbox (8KB)                                  */
    /*   __blackbox_start / __blackbox_end                        */
    /*                                                             */
    /* Sprint 12: WASM heap (64KB)                                */
    /*   __wasm_heap_start / __wasm_heap_end                      */
    /* ═══════════════════════════════════════════════════════════ */
}
