/* sipahi.ld — Sipahi Microkernel Linker Script
 * ====================================
 * Sipahi Memory Map (QEMU virt, RV64)
 *
 * 0x80000000  .text         (RX)   ~490KB  Kernel + Wasmi code
 * 0x800798xx  .rodata       (R)    ~84KB   Constants, Wasmi tables
 * 0x8008xxxx  .data         (RW)   ~150KB  Initialized data
 * 0x800xxxxx  .bss          (RW)   ~10KB   Statics (IPC, blackbox, config)
 *             kernel stack  (RW)   16KB    Kernel stack
 * __pmp_data_end ─── PMP Entry 5 TOR upper bound ───
 *             .task_stacks  (RW)   64KB    Per-task NAPOT PMP (Entry 8)
 *             .wasm_arena   (RW)   4MB     Wasmi bump allocator (M-mode only)
 * __clear_end / _end
 *
 * PMP Entry 4-5: [data_start, __pmp_data_end) RW L-bit
 * Task stacks: PMP match yok → U-mode DENY, per-task NAPOT Entry 8
 * WASM arena:  PMP match yok → U-mode DENY, M-mode erişir (spec)
 *
 * RAM: 8MB allocated (0x80000000-0x80800000), QEMU 512MB available
 */
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
    /* KRİTİK: boot.S'de __bss_start → __clear_end arası sıfırlanmalı!  */
    /* ALIGN(8): boot.S `sd` (8-byte store) + addi a0,a0,8 ile temizler */
    /* __bss_end de ALIGN(8): son bloğun taşmasını önler                 */
    .bss (NOLOAD) : ALIGN(8) {
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

    /* ═══ PMP Entry 5 TOR üst sınırı ═══ */
    /* Bundan öncesi: .data + .bss + kernel_stack → Entry 4-5 RW (L-bit) */
    __pmp_data_end = .;

    /* ═══ BUNDAN SONRASI U-MODE DENY ═══ */
    /* PMP match yok → U-mode erişemez (RISC-V spec §3.7.1) */
    /* M-mode erişir (PMP match yoksa M-mode izinli)         */

    /* Task stacks — per-task NAPOT PMP ile korunur (Entry 8) */
    .task_stacks (NOLOAD) : ALIGN(8192) {
        __task_stacks_start = .;
        *(.task_stacks)
        __task_stacks_end = .;
    } > RAM

    /* WASM arena — Wasmi interpreter M-mode'da erişir */
    .wasm_arena (NOLOAD) : ALIGN(4096) {
        __wasm_arena_start = .;
        *(.wasm_arena)
        __wasm_arena_end = .;
    } > RAM

    /* Boot clear loop: __bss_start → __clear_end
       BSS + kernel_stack + task_stacks + wasm_arena hepsini sıfırlar */
    . = ALIGN(8);
    __clear_end = .;
    _end = .;
}
