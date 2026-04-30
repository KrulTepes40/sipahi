# Sipahi Microkernel — Context Switch Assembly
# Sprint U-9: user_sp kaydı sabit trap frame adresinden (mscratch swap sonrası)
#
# switch_context(old_ctx: *mut TaskContext, new_ctx: *const TaskContext)
#   a0 = eski task'ın context pointer'ı (kaydet)
#   a1 = yeni task'ın context pointer'ı (yükle)
#
# 14 callee-saved + mepc + mstatus = 16 alan × 8 byte = 128 byte
#
# ÖNEMLİ: switch_context trap handler içinden çağrılıyor. sp = kernel_sp
# olmasına rağmen trap frame çok aşağıda DEĞİL — her Rust fonksiyonu
# kendi frame'ini itmiş. `sd sp, 8(a0)` YANLIŞ: kernel sp'sini kaydeder,
# user_sp'yi DEĞİL. `ld sp, 8(a1)` YANLIŞ: TaskContext.sp'den kernel sp
# yükler, ama bu task'ın user_sp'sidir.
#
# ÇÖZÜM: user_sp trap frame'inde kaydedilmiş — sabit adres __stack_top - 16
# (trap_entry: addi sp, sp, -272; user_sp offset 256 → __stack_top-272+256).
# switch_context bu sabit adresten okur/yazar. sp'ye dokunmaz.

.section .text
.global switch_context

switch_context:
    # ─── Eski task'ın register'larını kaydet ───
    sd      ra,    0(a0)

    # user_sp: trap frame'den oku (SABİT ADRES __stack_top - 16)
    # sp trap frame'i göstermiyor (Rust call zinciri itti)
    la      t0, __stack_top
    ld      t1, -16(t0)        # trap frame user_sp slot
    sd      t1,    8(a0)       # TaskContext.sp = user_sp

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

    # user_sp: TaskContext'ten trap frame'e yaz (SABİT ADRES __stack_top - 16)
    ld      t1,    8(a1)       # t1 = yeni task'ın user_sp
    la      t0, __stack_top
    sd      t1, -16(t0)        # trap frame user_sp slot güncelle

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
# switch_context ret → buraya gelir (fresh task'larda ra = task_trampoline).
# Bu yol trap.S `.restore_regs` exit sequence'ını ATLAR — dolayısıyla
# mscratch ve sp burada manuel restore edilmeli, aksi halde:
#   • mscratch = 0 (trap entry'de sıfırlandı, restore yok) → bir sonraki
#     trap entry `csrrw sp, mscratch, sp` ile sp=0 yakalar → .nested_fault
#   • sp = kernel_sp+offset (switch_context entry'sindeki kernel sp) →
#     U-mode task kernel stack alanını "user stack" olarak kullanır
#     (PMP entry 4 .data RW → kernel veri yapıları yazılabilir)
# Sprint U-18 GÖREV 1: task_trampoline mscratch=kernel_sp ve sp=user_sp
# atayarak start_first_task ile aynı invariant'ı kurar.
# user_sp trap frame slot'undan (__stack_top - 16) okunur — switch_context
# bu slotu yeni task'ın user_sp'siyle güncelledi.
# mepc ve mstatus zaten CSR'a yüklendi (yukarıdaki csrw).
.global task_trampoline
task_trampoline:
    la      t0, __stack_top
    ld      t1, -16(t0)        # t1 = user_sp (switch_context update etti)
    csrw    mscratch, t0       # mscratch = kernel_sp (= __stack_top)
    mv      sp, t1             # sp = user_sp (kernel sp leak engellenir)
    mret
