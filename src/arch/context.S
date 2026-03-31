# Sipahi Microkernel — Context Switch Assembly
# Sprint 4: Task register save/restore
#
# switch_context(old_ctx: *mut TaskContext, new_ctx: *const TaskContext)
#   a0 = eski task'ın context pointer'ı (kaydet)
#   a1 = yeni task'ın context pointer'ı (yükle)
#
# Callee-saved registers: ra, sp, s0-s11 (14 register × 8 byte = 112 byte)

.section .text
.global switch_context

switch_context:
    # ─── Eski task'ın register'larını kaydet ───
    sd      ra,   0(a0)
    sd      sp,   8(a0)
    sd      s0,  16(a0)
    sd      s1,  24(a0)
    sd      s2,  32(a0)
    sd      s3,  40(a0)
    sd      s4,  48(a0)
    sd      s5,  56(a0)
    sd      s6,  64(a0)
    sd      s7,  72(a0)
    sd      s8,  80(a0)
    sd      s9,  88(a0)
    sd      s10, 96(a0)
    sd      s11, 104(a0)

    # ─── Yeni task'ın register'larını yükle ───
    ld      ra,   0(a1)
    ld      sp,   8(a1)
    ld      s0,  16(a1)
    ld      s1,  24(a1)
    ld      s2,  32(a1)
    ld      s3,  40(a1)
    ld      s4,  48(a1)
    ld      s5,  56(a1)
    ld      s6,  64(a1)
    ld      s7,  72(a1)
    ld      s8,  80(a1)
    ld      s9,  88(a1)
    ld      s10, 96(a1)
    ld      s11, 104(a1)

    # ra yüklendi → ret yeni task'ın ra'sına atlar
    ret
