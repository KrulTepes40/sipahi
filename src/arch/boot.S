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

    # 2. BSS bölgesini sıfırla (bare metal'de otomatik sıfırlanmaz!)
    # ~4MB (WASM arena dahil). CVA6 100MHz: ~4ms, QEMU 10MHz: ~40ms
    # Aviyonik boot bütçesinde kabul edilebilir.
    # Alternatif: WASM arena'yı .noinit section'a taşıyıp
    # BumpAllocator::epoch_reset() ile lazy-zero (v2.0)
    la      a0, __bss_start
    la      a1, __bss_end
.bss_loop:
    bgeu    a0, a1, .bss_done
    sd      zero, (a0)
    addi    a0, a0, 8
    j       .bss_loop
.bss_done:

    # 3. Stack pointer'ı ayarla
    la      sp, __stack_top

    # 4. Rust'a atla (geri dönmez)
    call    rust_main

    # Rust'tan dönerse (olmamalı) → sonsuz döngü
    j       .park

.park:
    wfi
    j       .park
