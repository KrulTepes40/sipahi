# Sipahi Microkernel — Trap Entry Assembly
# Sprint 7: ecall syscall dispatch desteği
#
# trap_handler(mcause, mepc, syscall_id, arg0, arg1, arg2, arg3) -> usize
#   a0 = mcause
#   a1 = mepc (ecall için +4 ilerletilmiş)
#   a2 = orijinal a7 (syscall ID)
#   a3 = orijinal a0 (arg0)
#   a4 = orijinal a1 (arg1)
#   a5 = orijinal a2 (arg2)
#   a6 = orijinal a3 (arg3)
#
# ecall: trap_handler dönüşü → saved a0'a yazılır (syscall sonucu)
# interrupt: saved a0 korunur (orijinal değer geri yüklenir)

.section .text
.global trap_entry
.align 4

trap_entry:
    # ─── Register Save ───
    addi    sp, sp, -256

    sd      ra,   0(sp)
    sd      t0,   8(sp)
    sd      t1,  16(sp)
    sd      t2,  24(sp)
    sd      a0,  32(sp)     # orijinal a0
    sd      a1,  40(sp)     # orijinal a1
    sd      a2,  48(sp)     # orijinal a2
    sd      a3,  56(sp)     # orijinal a3
    sd      a4,  64(sp)
    sd      a5,  72(sp)
    sd      a6,  80(sp)
    sd      a7,  88(sp)     # orijinal a7 (syscall ID)
    sd      t3,  96(sp)
    sd      t4, 104(sp)
    sd      t5, 112(sp)
    sd      t6, 120(sp)

    # CSR'ları oku
    csrr    t0, mcause
    csrr    t1, mepc
    sd      t0, 128(sp)     # mcause kaydet
    sd      t1, 136(sp)     # mepc kaydet (henüz +4 yok)

    # ─── ecall kontrolü: mcause == 8 (U-mode) veya 11 (M-mode) ───
    # ecall ise mepc'yi +4 ilerlet (ecall = 4 byte instruction)
    li      t2, 8
    beq     t0, t2, .ecall_advance
    li      t2, 11
    beq     t0, t2, .ecall_advance
    j       .call_handler

.ecall_advance:
    addi    t1, t1, 4
    sd      t1, 136(sp)     # mepc+4'ü stack'e kaydet

.call_handler:
    # ─── 7 argüman hazırla ───
    # a0 = mcause
    # a1 = mepc (ecall için +4)
    # a2 = orijinal a7 (syscall ID)
    # a3 = orijinal a0 (arg0)
    # a4 = orijinal a1 (arg1)
    # a5 = orijinal a2 (arg2)
    # a6 = orijinal a3 (arg3)
    mv      a0, t0          # mcause
    mv      a1, t1          # mepc (veya mepc+4)
    ld      a2, 88(sp)      # orijinal a7
    ld      a3, 32(sp)      # orijinal a0
    ld      a4, 40(sp)      # orijinal a1
    ld      a5, 48(sp)      # orijinal a2
    ld      a6, 56(sp)      # orijinal a3

    call    trap_handler

    # ─── ecall: dönüş değerini saved a0'a yaz ───
    # interrupt: saved a0 korunur (orijinal değer geri yüklenir)
    ld      t0, 128(sp)     # mcause'ı tekrar oku
    li      t1, 8
    beq     t0, t1, .write_result
    li      t1, 11
    beq     t0, t1, .write_result
    j       .restore_regs

.write_result:
    sd      a0, 32(sp)      # syscall sonucunu saved a0 slot'una yaz

.restore_regs:
    # ─── mepc geri yükle (ecall: +4, interrupt: orijinal) ───
    ld      t0, 136(sp)
    csrw    mepc, t0

    ld      ra,   0(sp)
    ld      t0,   8(sp)
    ld      t1,  16(sp)
    ld      t2,  24(sp)
    ld      a0,  32(sp)     # orijinal a0 veya syscall sonucu
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
