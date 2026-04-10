# Sipahi Microkernel — Context Switch Assembly
# U-mode altyapısı: Task register save/restore + mepc/mstatus
#
# switch_context(old_ctx: *mut TaskContext, new_ctx: *const TaskContext)
#   a0 = eski task'ın context pointer'ı (kaydet)
#   a1 = yeni task'ın context pointer'ı (yükle)
#
# 14 callee-saved + mepc + mstatus = 16 alan × 8 byte = 128 byte

.section .text
.global switch_context

switch_context:
    # ─── Eski task'ın register'larını kaydet ───
    sd      ra,    0(a0)
    sd      sp,    8(a0)
    sd      s0,   16(a0)
    sd      s1,   24(a0)
    sd      s2,   32(a0)
    sd      s3,   40(a0)
    sd      s4,   48(a0)
    sd      s5,   56(a0)
    sd      s6,   64(a0)
    sd      s7,   72(a0)
    sd      s8,   80(a0)
    sd      s9,   88(a0)
    sd      s10,  96(a0)
    sd      s11, 104(a0)
    csrr    t0, mepc
    sd      t0,  112(a0)
    csrr    t0, mstatus
    sd      t0,  120(a0)

    # ─── Yeni task'ın register'larını yükle ───
    ld      ra,    0(a1)
    ld      sp,    8(a1)
    ld      s0,   16(a1)
    ld      s1,   24(a1)
    ld      s2,   32(a1)
    ld      s3,   40(a1)
    ld      s4,   48(a1)
    ld      s5,   56(a1)
    ld      s6,   64(a1)
    ld      s7,   72(a1)
    ld      s8,   80(a1)
    ld      s9,   88(a1)
    ld      s10,  96(a1)
    ld      s11, 104(a1)
    ld      t0,  112(a1)
    csrw    mepc, t0
    ld      t0,  120(a1)
    csrw    mstatus, t0

    # ra yüklendi → ret yeni task'ın ra'sına atlar
    ret

# ─── U-mode task trampoline ───
# switch_context ret → buraya gelir (fresh task'larda ra = task_trampoline)
# mepc ve mstatus zaten CSR'a yüklendi (yukarıdaki csrw)
# mret → mepc (task entry), MPP=U → U-mode, MPIE→MIE
.global task_trampoline
task_trampoline:
    mret
