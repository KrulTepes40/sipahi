# Sipahi Microkernel — Boot Assembly
# Sprint 1: _start → BSS sıfırla → stack kur → Rust'a atla
#
# QEMU virt: -bios none ile 0x80000000'e boot eder
# a0 = hartid (QEMU ZSBL'den), a1 = DTB adresi (şimdi kullanılmıyor)

.section .text.boot
.global _start

_start:
    # 1. Hart 0 dışındakileri park et (güvenlik)
    csrr    a0, mhartid
    bnez    a0, .park

    # 2. BSS + task stacks + WASM arena sıfırla
    # __bss_start → __clear_end: ~4.1MB (WASM arena dahil)
    # CVA6 100MHz: ~4ms, QEMU 10MHz: ~40ms
    # Aviyonik boot bütçesinde kabul edilebilir.
    la      a0, __bss_start
    la      a1, __clear_end
.bss_loop:
    bgeu    a0, a1, .bss_done
    sd      zero, (a0)
    addi    a0, a0, 8
    j       .bss_loop
.bss_done:

    # 3. Stack pointer'ı ayarla
    la      sp, __stack_top

    # 4. mscratch = kernel stack top (Sprint U-9: trap handler swap uses this)
    csrw    mscratch, sp

    # 5. Rust'a atla (geri dönmez)
    call    rust_main

    # Rust'tan dönerse (olmamalı) → sonsuz döngü
    j       .park

.park:
    wfi
    j       .park
