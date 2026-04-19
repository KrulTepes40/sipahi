# Sipahi Microkernel — Trap Entry Assembly
# Sprint U-9: mscratch swap + nested trap detection
#
# mscratch = kernel_sp (boot'ta init, trap exit'te restore)
# Trap entry: atomic swap sp↔mscratch → handler kernel stack'te çalışır
# Nested trap: mscratch=0 iken trap → sp=0 → beqz yakalar → park
#
# trap_handler(mcause, mepc, syscall_id, arg0, arg1, arg2, arg3) -> usize

.section .text
.global trap_entry
.align 4

trap_entry:
    # ─── mscratch swap: sp = kernel_sp, mscratch = user_sp ───
    csrrw   sp, mscratch, sp
    beqz    sp, .nested_fault       # mscratch 0 idi → nested trap

    # Frame: 272 byte (256 register save + 8 user_sp + 8 padding)
    # 16-byte aligned (272 = 17 × 16)
    addi    sp, sp, -272

    # t0'ı ÖNCE kaydet (henüz clobber olmadı, user değeri)
    sd      t0,   8(sp)

    # user_sp'yi mscratch'tan oku ve frame'e kaydet
    csrr    t0, mscratch            # t0 = user_sp
    sd      t0, 256(sp)             # offset 256: user_sp slot

    # mscratch'ı sıfırla — nested trap detection armed
    csrw    mscratch, zero

    # ─── Kalan register'ları kaydet ───
    sd      ra,   0(sp)
    # t0 zaten 8(sp)'de
    sd      t1,  16(sp)
    sd      t2,  24(sp)
    sd      a0,  32(sp)             # orijinal a0
    sd      a1,  40(sp)             # orijinal a1
    sd      a2,  48(sp)             # orijinal a2
    sd      a3,  56(sp)             # orijinal a3
    sd      a4,  64(sp)
    sd      a5,  72(sp)
    sd      a6,  80(sp)
    sd      a7,  88(sp)             # orijinal a7 (syscall ID)
    sd      t3,  96(sp)
    sd      t4, 104(sp)
    sd      t5, 112(sp)
    sd      t6, 120(sp)

    # CSR'ları oku
    csrr    t0, mcause
    csrr    t1, mepc
    sd      t0, 128(sp)             # mcause
    sd      t1, 136(sp)             # mepc (henüz +4 yok)

    # ─── ecall kontrolü: mcause == 8 (U-mode) veya 11 (M-mode) ───
    li      t2, 8
    beq     t0, t2, .ecall_advance
    li      t2, 11
    beq     t0, t2, .ecall_advance
    j       .call_handler

.ecall_advance:
    addi    t1, t1, 4
    sd      t1, 136(sp)             # mepc+4

.call_handler:
    # ─── 7 argüman hazırla ───
    mv      a0, t0                  # mcause
    mv      a1, t1                  # mepc (veya mepc+4)
    ld      a2, 88(sp)              # orijinal a7
    ld      a3, 32(sp)              # orijinal a0
    ld      a4, 40(sp)              # orijinal a1
    ld      a5, 48(sp)              # orijinal a2
    ld      a6, 56(sp)              # orijinal a3

    call    trap_handler

    # ─── ecall: dönüş değerini saved a0'a yaz ───
    ld      t0, 128(sp)             # mcause tekrar oku
    li      t1, 8
    beq     t0, t1, .write_result
    li      t1, 11
    beq     t0, t1, .write_result
    j       .restore_regs

.write_result:
    sd      a0, 32(sp)              # syscall sonucunu saved a0'a yaz

.restore_regs:
    # ─── mepc geri yükle ───
    ld      t0, 136(sp)
    csrw    mepc, t0

    # ─── Register restore (t0 HARİÇ — exit sequence'de kullanılacak) ───
    ld      ra,   0(sp)
    # t0 aşağıda restore edilecek
    ld      t1,  16(sp)
    ld      t2,  24(sp)
    ld      a0,  32(sp)             # orijinal a0 veya syscall sonucu
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

    # ─── mscratch restore + user_sp swap ───
    # 1. user_sp'yi frame'den oku → mscratch'a koy (geçici)
    ld      t0, 256(sp)             # t0 = user_sp
    csrw    mscratch, t0            # mscratch = user_sp (geçici)

    # 2. user t0'ı restore et (artık mscratch'a ihtiyacımız yok)
    ld      t0,   8(sp)             # t0 = user t0 ✓

    # 3. Frame'i temizle → sp = kernel_sp top
    addi    sp, sp, 272

    # 4. Atomic swap: sp = user_sp, mscratch = kernel_sp
    csrrw   sp, mscratch, sp        # sp=user_sp, mscratch=kernel_sp ✓

    mret

# ─── Nested fault handler ───
# mscratch 0 idi → trap handler içinde tekrar trap geldi
# sp = 0 (swap sonucu). Stack yok, UART bile çağırma.
# Direkt park — production'da watchdog timer reset yapacak.
.nested_fault:
    j       .park_nested

.park_nested:
    wfi
    j       .park_nested
