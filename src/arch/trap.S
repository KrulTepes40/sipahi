# Sipahi Microkernel — Trap Entry Assembly
# Sprint 2-4: Interrupt/exception entry + context switch desteği
#
# KRİTİK: mepc mret öncesi geri yüklenmeli!
# Context switch sonrası yanlış task'a dönmeyi önler.

.section .text
.global trap_entry
.align 4

trap_entry:
    # ─── Register Save (caller-saved + argümanlar) ───
    addi    sp, sp, -256

    sd      ra,   0(sp)
    sd      t0,   8(sp)
    sd      t1,  16(sp)
    sd      t2,  24(sp)
    sd      a0,  32(sp)
    sd      a1,  40(sp)
    sd      a2,  48(sp)
    sd      a3,  56(sp)
    sd      a4,  64(sp)
    sd      a5,  72(sp)
    sd      a6,  80(sp)
    sd      a7,  88(sp)
    sd      t3,  96(sp)
    sd      t4, 104(sp)
    sd      t5, 112(sp)
    sd      t6, 120(sp)

    # mcause ve mepc kaydet
    csrr    a0, mcause
    csrr    a1, mepc
    sd      a0, 128(sp)
    sd      a1, 136(sp)

    # ─── Rust trap handler çağır ───
    # a0 = mcause, a1 = mepc (zaten ayarlı)
    call    trap_handler

    # ─── Register Restore ───
    # KRİTİK: mepc'yi ÖNCE geri yükle
    # Context switch sonrası doğru task'a dönmek için zorunlu
    ld      t0, 136(sp)
    csrw    mepc, t0

    ld      ra,   0(sp)
    ld      t0,   8(sp)
    ld      t1,  16(sp)
    ld      t2,  24(sp)
    ld      a0,  32(sp)
    ld      a1,  40(sp)
    ld      a2,  48(sp)
    ld      a3,  56(sp)
    ld      a4,  64(sp)
    ld      a5,  72(sp)
    ld      a6,  80(sp)
    ld      a7,  88(sp)
    ld      t3,  96(sp)
    ld      t4, 104(sp)
    ld      t5, 112(sp)
    ld      t6, 120(sp)

    addi    sp, sp, 256

    mret
