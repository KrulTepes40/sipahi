/* sipahi.ld — Sipahi Microkernel Linker Script */
/* QEMU virt machine, RV64, RAM @ 0x80000000   */

ENTRY(_start)

MEMORY {
    RAM (rwx) : ORIGIN = 0x80000000, LENGTH = 512K
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
    /* KRİTİK: boot.S'de __bss_start → __bss_end arası sıfırlanmalı! */
    .bss : ALIGN(4) {
        __bss_start = .;
        *(.bss .bss.*)
        *(COMMON)
        __bss_end = .;
    } > RAM

    /* Kernel stack (4KB, R1'in parçası) */
    . = ALIGN(16);
    __stack_bottom = .;
    . += 4096;
    __stack_top = .;

    /* Stack guard — PMP ile korunacak */
    __stack_guard = __stack_bottom;

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
