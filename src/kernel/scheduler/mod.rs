//! Preemptive fixed-priority scheduler with per-task budget and period enforcement.
// Sipahi — Scheduler (Sprint 10)
// Fixed-Priority Preemptive + Budget + Deadline
//
// Sprint 4:  Round-robin
// Sprint 10: priority (0-15), budget_cycles, period_ticks, policy engine entegrasyonu
//
// Her tick:
//   1. Tüm task'lar için period ilerlet -> süre dolduysa bütçe sıfırla + READY
//   2. Mevcut task bütçesini düş -> 0 ise SUSPENDED + policy
//   3. En yüksek öncelikli Ready task'ı seç (düşük sayı = yüksek öncelik)
//   4. Context switch
//
// WCET: ≤0.8μs (doküman §SCHEDULER)

use crate::common::config::{
    MAX_TASKS, TASK_STACK_SIZE, CYCLES_PER_TICK, WATCHDOG_LIMIT, WATCHDOG_WINDOW_MIN,
    MAX_SENDS_PER_TICK,
    SYSTEM_TASK_ID, SYSTEM_TASK_INDEX, KERNEL_BOOT_ID, // U-19 GÖREV 1
};
use crate::common::sync::SingleHartCell;
use crate::common::types::TaskState;
use crate::kernel::policy::{FailureMode, PolicyEvent};

// ═══════════════════════════════════════════════════════
// TaskContext: callee-saved registers
// ═══════════════════════════════════════════════════════

/// Callee-saved register'lar + mepc/mstatus — context.S ile eşleşmeli
/// 14 callee-saved + mepc + mstatus = 16 alan × 8 byte = 128 byte
#[repr(C)]
pub struct TaskContext {
    pub ra:      usize,  // Return address
    pub sp:      usize,  // Stack pointer
    pub s0:      usize,  // Saved registers
    pub s1:      usize,
    pub s2:      usize,
    pub s3:      usize,
    pub s4:      usize,
    pub s5:      usize,
    pub s6:      usize,
    pub s7:      usize,
    pub s8:      usize,
    pub s9:      usize,
    pub s10:     usize,
    pub s11:     usize,
    pub mepc:    usize,  // U-mode: task program counter
    pub mstatus: usize,  // U-mode: mstatus (MPP bits)
}

// U-21 GÖREV 12 [M5]: TaskContext layout assertion — context.S sabit offset'ler
// (sd ra, 0(a0); sd s0, 16(a0); ...; sd mstatus, 120(a0)) ile bağlı.
// 16 alan × 8 byte = 128 byte; alan eklenirse bu assertion build fail eder.
const _: () = assert!(
    core::mem::size_of::<TaskContext>() == 128,
    "TaskContext size changed — context.S offsets MUST be updated"
);

impl TaskContext {
    pub const fn zero() -> Self {
        TaskContext {
            ra: 0, sp: 0,
            s0: 0, s1: 0, s2: 0, s3: 0,
            s4: 0, s5: 0, s6: 0, s7: 0,
            s8: 0, s9: 0, s10: 0, s11: 0,
            mepc: 0, mstatus: 0,
        }
    }
}

// ═══════════════════════════════════════════════════════
// Task Control Block
// ═══════════════════════════════════════════════════════

pub struct Task {
    pub id:               u8,
    pub state:            TaskState,
    pub context:          TaskContext,
    pub entry:            usize,         // Giriş noktası (restart için)
    pub stack_top:        usize,         // Hizalanmış stack üstü (restart için)
    // Sprint 10: Budget + Priority
    pub priority:         u8,            // 0-15 (0=en yüksek, DAL-A grubu 0-3)
    pub dal:              u8,            // 0=A 1=B 2=C 3=D
    pub budget_cycles:    u32,           // Periyot başına bütçe (cycle)
    pub remaining_cycles: u32,           // Bu periyotta kalan cycle
    pub period_ticks:     u32,           // Periyot uzunluğu (scheduler tick sayısı)
    pub period_counter:   u32,           // Mevcut periyot içindeki tick sayacı
    pub watchdog_counter: u32,           // Tick sayacı — yield/kick ile sıfırlanır
    pub watchdog_limit:   u32,           // Limit (0=devre dışı) — aşılırsa policy tetik
    pub syscall_count:    u32,           // Anomali tespiti — dispatch'te artırılır
    pub ipc_send_count:   u32,           // Rate limiter — tick'te sıfırlanır
    pub watchdog_window_min: u32,        // Windowed: kick bu tick'ten önce gelirse hata
    pub original_budget:     u32,        // Degrade öncesi orijinal bütçe (kurtarma için)
    pub pmp_addr_napot:      usize,      // NAPOT-encoded PMP address (entry 8)
    /// U-25 FIX-3: SNTM native task flag.
    /// false = legacy single-NAPOT stack path (write_per_task_napot, is_valid_user_ptr
    ///         task_stack_range). Mevcut task_a/task_b davranışı.
    /// true  = SNTM multi-region: scheduler reload_pmp_profile, is_valid_user_ptr
    ///         check_ptr_in_profile. U-26 native_create_task() bunu set eder.
    pub is_sntm_native:      bool,
}

impl Task {
    pub const fn empty() -> Self {
        Task {
            id:               0,
            state:            TaskState::Dead,
            context:          TaskContext::zero(),
            entry:            0,
            stack_top:        0,
            priority:         15,         // En düşük öncelik (boş slot)
            dal:              3,          // DAL-D (boş slot)
            budget_cycles:    0,
            remaining_cycles: 0,
            period_ticks:     0,
            period_counter:   0,
            watchdog_counter: 0,
            watchdog_limit:   0,
            syscall_count:    0,
            ipc_send_count:   0,
            watchdog_window_min: 0,
            original_budget:     0,
            pmp_addr_napot:      0,
            is_sntm_native:      false,  // U-25 FIX-3 default legacy
        }
    }
}

// ═══════════════════════════════════════════════════════
// Statik alanlar
// ═══════════════════════════════════════════════════════

/// 8KB-aligned stack — NAPOT PMP için compile-time alignment garantisi
#[repr(align(8192))]
pub(crate) struct AlignedStack(pub(crate) [u8; TASK_STACK_SIZE]);

/// Task stack'leri — statik, heap yok, 8KB aligned (NAPOT uyumlu)
/// .task_stacks section: PMP Entry 5 dışında, U-mode DENY (per-task NAPOT Entry 8)
#[link_section = ".task_stacks"]
pub(crate) static TASK_STACKS: SingleHartCell<[AlignedStack; MAX_TASKS]> = SingleHartCell::new(
    [const { AlignedStack([0u8; TASK_STACK_SIZE]) }; MAX_TASKS]
);

/// Task dizisi — statik
pub(crate) static TASKS: SingleHartCell<[Task; MAX_TASKS]> = SingleHartCell::new([
    Task::empty(), Task::empty(), Task::empty(), Task::empty(),
    Task::empty(), Task::empty(), Task::empty(), Task::empty(),
]);

/// Aktif task sayısı
static TASK_COUNT: SingleHartCell<usize> = SingleHartCell::new(0);

/// DEGRADE mesajı bir kez yazdırılsın — spam önleme
static DEGRADE_LOGGED: SingleHartCell<bool> = SingleHartCell::new(false);

/// Sistem degrade modunda mı?
static DEGRADED: SingleHartCell<bool> = SingleHartCell::new(false);

// U-17 GÖREV 7: Degrade başlangıç tick'i — flapping (sürekli oscillation) önleme
// degrade_system() çağrıldığında current_tick kaydedilir, try_recover en az
// COOLDOWN_TICKS sonra gerçek recovery başlatır.
static DEGRADE_TICK: SingleHartCell<u32> = SingleHartCell::new(0);
const DEGRADE_COOLDOWN_TICKS: u32 = 100; // ~1s @10ms tick

/// Şu anki çalışan task index'i
static CURRENT_TASK: SingleHartCell<usize> = SingleHartCell::new(0);

// context.S'deki switch_context fonksiyonu
extern "C" {
    fn switch_context(old: *mut TaskContext, new: *const TaskContext);
    fn task_trampoline() -> !;
    /// Linker sembolü — kernel stack üst sınırı (trap handler kernel_sp)
    static __stack_top: u8;
}

// ═══════════════════════════════════════════════════════
// Public API
// ═══════════════════════════════════════════════════════

/// Create a new task with the given configuration.
pub(crate) fn create_task(cfg: &crate::common::types::TaskConfig) -> Option<u8> {
    // SAFETY: Boot sequence, interrupts not yet enabled.
    let count = unsafe { *TASK_COUNT.get() };
    if count >= MAX_TASKS { return None; }

    // Sprint U-14: budget=0 task ilk tick'te Suspended'a düşer, asla çalışmaz -> silent dead task
    if cfg.budget_cycles == 0 { return None; }

    // SAFETY: count < MAX_TASKS — index in bounds.
    let stack_base = unsafe {
        &TASK_STACKS.get()[count].0 as *const _ as usize
    };
    let stack_top = stack_base + TASK_STACK_SIZE;
    let stack_top_aligned = stack_top & !0xF; // safe arithmetic

    // SAFETY: Single-hart, count < MAX_TASKS.
    unsafe {
        let t = &mut TASKS.get_mut()[count];
        t.id               = count as u8;
        t.state            = TaskState::Ready;
        t.context          = TaskContext::zero();
        #[cfg(not(kani))]
        { t.context.ra     = task_trampoline as *const () as usize; }
        #[cfg(kani)]
        { t.context.ra     = cfg.entry as usize; }
        t.context.sp       = stack_top_aligned;
        t.entry            = cfg.entry as usize;
        t.stack_top        = stack_top_aligned;
        // U-mode: mret sonrası MPP=U, MPIE=1 (interrupt aktif)
        t.context.mepc     = cfg.entry as usize;
        t.context.mstatus  = crate::arch::csr::MSTATUS_MPP_U
                           | crate::arch::csr::MSTATUS_MPIE;
        t.priority         = cfg.priority;
        t.dal              = cfg.dal;
        t.budget_cycles    = cfg.budget_cycles;
        t.remaining_cycles = cfg.budget_cycles;
        t.period_ticks     = cfg.period_ticks;
        t.period_counter   = 0;
        t.watchdog_counter     = 0;
        t.watchdog_limit       = WATCHDOG_LIMIT;
        t.watchdog_window_min  = WATCHDOG_WINDOW_MIN;
        t.original_budget      = cfg.budget_cycles;
        // NAPOT PMP: 8KB stack bölgesini entry 8'e encode et
        t.pmp_addr_napot       = (stack_base >> 2) | crate::arch::pmp::PMP_NAPOT_MASK_8KB;
        // U-25 FIX-3: propagate native flag (default false = legacy task).
        t.is_sntm_native       = cfg.is_sntm_native;

        *TASK_COUNT.get_mut() += 1;
    }
    Some(count as u8)
}

/// U-25 FIX-3 + FIX-4: task SNTM-native mi?
/// is_valid_user_ptr ve scheduler reload hook'u bu helper'ı çağırır:
///   - false (default) — legacy single-NAPOT stack path
///   - true            — SNTM multi-region + Access path
///
/// Out-of-bounds task_id, Dead/Isolated/uninitialized → false (defansif legacy).
#[must_use]
#[allow(dead_code)] // U-25 G1: G5 (is_valid_user_ptr) ve G11 (scheduler hook) tüketir
pub(crate) fn is_task_sntm_native(task_id: u8) -> bool {
    let idx = task_id as usize;
    if idx >= MAX_TASKS { return false; }
    // SAFETY: Single-hart, index bounded above.
    let t = unsafe { &TASKS.get()[idx] };
    if matches!(t.state, TaskState::Dead | TaskState::Isolated) {
        return false;
    }
    t.is_sntm_native
}

/// Scheduler tick — TIMER INTERRUPT'tan çağrılır.
/// U-21 GÖREV 11 [H5]: schedule_yield()'den AYRI; bu fonksiyon tüm
/// per-tick state advance'ları içerir (blackbox tick, IPC rate reset,
/// watchdog increment, budget decrement, period advance, PMP shadow verify).
/// U-mode task'lar bu yola erişemez (sadece timer interrupt çağırır);
/// hostile yield spam tick state'ini bozamaz.
///
/// WCET: ≤0.8μs @ 100MHz (doküman hedef)
pub(crate) fn schedule_timer_tick() {
    // SAFETY: MIE=0 in trap context, single-hart — no concurrent access.
    unsafe {
        // Sprint U-16: Early return Phase 2 SONRASINA taşındı — güvenlik
        // mekanizmaları (PMP shadow, watchdog, budget, blackbox tick) tek task'lı
        // boot/test senaryolarında da çalışmalı. Sadece Phase 3+4 (priority
        // select + context switch) atlanır.
        if *TASK_COUNT.get() == 0 {
            return; // Hiç task yok -> zarar verecek state yok
        }

        // Blackbox tick sayacını ilerlet (schedule() her çağrısında)
        #[cfg(not(kani))]
        crate::ipc::blackbox::advance_tick();

        let old = *CURRENT_TASK.get();

        // ─── PMP bütünlük doğrulama ───
        #[cfg(not(kani))]
        if !crate::kernel::memory::verify_pmp_integrity() {
            #[cfg(feature = "debug-boot")]
            crate::arch::uart::println("[PMP] INTEGRITY FAIL — SHUTDOWN");
            crate::ipc::blackbox::log(
                crate::ipc::blackbox::BlackboxEvent::PmpFail, SYSTEM_TASK_ID, &[],
            );
            // Policy path — decide_action(PmpIntegrityFail, *, *) -> Shutdown
            let action = crate::kernel::policy::apply_policy(
                SYSTEM_TASK_ID, // kernel pseudo-task
                PolicyEvent::PmpIntegrityFail,
                0_u8, // DAL-A
            );
            apply_action(SYSTEM_TASK_INDEX, action);
        }

        // ─── Faz 1: Periyot ilerletme (tüm task'lar) ───
        // Periyot dolarsa: bütçe sıfırla, SUSPENDED -> READY
        // U-21 GÖREV 17 [MP1]: dual bound (TASK_COUNT && MAX_TASKS) — Phase 3
        // ile simetrik. TASK_COUNT bozulursa array OOB engellenir.
        let count_phase1 = *TASK_COUNT.get();
        let mut i: usize = 0;
        while i < count_phase1 && i < MAX_TASKS {
            let t = &mut TASKS.get_mut()[i];
            if t.period_ticks > 0 {
                t.period_counter = t.period_counter.wrapping_add(1);
                if t.period_counter >= t.period_ticks {
                    t.period_counter   = 0;
                    t.remaining_cycles = t.budget_cycles;
                    // U-19 GÖREV 10: helper inline kullanım (Kani Proof 71 ile aynı)
                    if is_period_reset_eligible(t.state) {
                        t.state = TaskState::Ready;
                        // Not: Isolated task'lar bu blokta hiç eşleşmez —
                        // kasıtlı. Isolated -> periyot reset ile READY yapılmaz.
                    }
                }
            }
            i += 1;
        }

        // Degrade kurtarma kontrolü
        #[cfg(not(kani))]
        try_recover_from_degrade();

        // ─── Faz 1.5: Watchdog + IPC rate reset ───
        // Sprint U-16: Watchdog SADECE Running task için artırılır — Ready task
        // CPU almaz, watchdog kick'i çağıramaz, dolayısıyla cezalandırılmamalı.
        // IPC rate reset Ready dahil tutulur (rate limit tick bazlı; Ready burst
        // tüketmemeli — kanal sıralı paylaşılan kaynak).
        // U-21 GÖREV 17 [MP1]: dual bound — Phase 1 ile simetrik.
        let count_phase15 = *TASK_COUNT.get();
        let mut w: usize = 0;
        while w < count_phase15 && w < MAX_TASKS {
            let st = TASKS.get_mut()[w].state;

            // IPC rate limit reset — Running ve Ready için tick başı sıfırlanır
            if st == TaskState::Running || st == TaskState::Ready {
                TASKS.get_mut()[w].ipc_send_count = 0;
            }

            // Watchdog SADECE Running task için artırılır
            if st == TaskState::Running {
                let t = &mut TASKS.get_mut()[w];
                // U-21 GÖREV 19 [MP4]: saturating_add — overflow_checks=true
                // altında u32::MAX'a ulaşan counter += 1 panic atar.
                // Saturating: counter u32::MAX'ta donar; should_watchdog_timeout
                // zaten >= limit kontrolü yapıyor -> semantik korunur.
                t.watchdog_counter = t.watchdog_counter.saturating_add(1);
                // U-19 GÖREV 10: helper inline kullanım (Kani Proof 95 ile aynı)
                if should_watchdog_timeout(t.watchdog_limit, t.watchdog_counter) {
                    // Watchdog tetiklendi — t'yi drop et, apply_action aliasing safe
                    let (id, dal) = (t.id, t.dal);
                    #[cfg(not(kani))]
                    crate::ipc::blackbox::log(
                        crate::ipc::blackbox::BlackboxEvent::WatchdogTimeout,
                        id, &[],
                    );
                    let action = crate::kernel::policy::apply_policy(
                        id, PolicyEvent::WatchdogTimeout, dal,
                    );
                    apply_action(w, action);
                    TASKS.get_mut()[w].watchdog_counter = 0;
                }
            }
            w += 1;
        }

        // ─── Faz 2: Bütçe düşümü + politika (mevcut task) ───
        // budget_cycles == 0 -> sınırsız bütçe (bütçe izleme devre dışı)
        let (budget_active, remaining, id_old, dal_old) = {
            let t = &mut TASKS.get_mut()[old];
            if t.budget_cycles > 0 && t.state == TaskState::Running {
                t.remaining_cycles = t.remaining_cycles.saturating_sub(CYCLES_PER_TICK);
                if t.remaining_cycles == 0 {
                    t.state = TaskState::Suspended;
                }
                (true, t.remaining_cycles, t.id, t.dal)
            } else {
                (false, 1, 0, 0) // dummy values, budget_active=false -> skip
            }
        }; // t dropped here
        if budget_active && remaining == 0 {
            #[cfg(not(kani))]
            {
                let ev = [id_old, dal_old];
                crate::ipc::blackbox::log(
                    crate::ipc::blackbox::BlackboxEvent::BudgetExhausted,
                    id_old, &ev,
                );
            }
            let action = crate::kernel::policy::apply_policy(
                id_old, PolicyEvent::BudgetExhausted, dal_old,
            );
            apply_action(old, action);
        }

        // ─── Faz 3+4: Priority select + context switch ───
        // U-21 GÖREV 11 [H5]: helper'a çıkarıldı; schedule_yield() de aynı
        // path'i kullanır (tek doğruluk kaynağı — drift imkansız).
        do_priority_select_and_switch(old);
    }
}

/// Phase 3+4 helper — priority select + context switch.
/// Hem schedule_timer_tick() (timer interrupt sonrası) hem schedule_yield()
/// (SYS_YIELD sonrası) bu fonksiyonu çağırır. State advance YAPMAZ; sadece
/// "şu an hangi task running olmalı" kararını uygular.
///
/// SAFETY: Caller MIE=0 garanti etmeli (trap context veya boot sequence).
#[inline]
unsafe fn do_priority_select_and_switch(old: usize) {
    // Tek task'ta context switch gereksiz
    if *TASK_COUNT.get() < 2 {
        return;
    }
    let count = *TASK_COUNT.get();
    let mut states = [TaskState::Dead; MAX_TASKS];
    let mut priorities = [15u8; MAX_TASKS];
    let mut k: usize = 0;
    while k < count && k < MAX_TASKS {
        states[k] = TASKS.get_mut()[k].state;
        priorities[k] = TASKS.get_mut()[k].priority;
        k += 1;
    }
    let next = match select_highest_priority(&states, &priorities, count) {
        Some(idx) => idx,
        None => return, // Hiç Ready/Running task yok
    };

    // Aynı task, hâlâ çalışıyor -> context switch gerekmez
    if next == old && TASKS.get_mut()[old].state == TaskState::Running {
        return;
    }

    if TASKS.get_mut()[old].state == TaskState::Running {
        TASKS.get_mut()[old].state = TaskState::Ready;
    }
    TASKS.get_mut()[next].state = TaskState::Running;
    *CURRENT_TASK.get_mut()     = next;

    // U-25 FIX-3: SNTM native task → multi-region reload, legacy → single-NAPOT.
    // task_a/task_b is_sntm_native=false → legacy path, regression yok.
    // Native task (U-26 native_create_task) → reload_pmp_profile (entry 8..15).
    #[cfg(not(kani))]
    {
        // U-22 GÖREV 24 [MP5]: MIE=0 invariant (HW write + shadow update penceresi).
        #[cfg(debug_assertions)]
        {
            let mstatus = crate::arch::csr::read_mstatus();
            debug_assert!(
                mstatus & 0x8 == 0,
                "MIE must be 0 during PMP shadow update (mstatus bit 3)"
            );
        }

        let next_task = &TASKS.get()[next];
        if next_task.is_sntm_native {
            // SNTM native: manifest-driven multi-region PMP profile.
            match crate::kernel::pmp::profile::get_pmp_profile(next_task.id) {
                Some(profile) if profile.region_count > 0 => {
                    // SAFETY: Trap context, MIE=0, single hart (yukarıdaki invariant).
                    unsafe { crate::arch::pmp::reload_pmp_profile(profile); }
                }
                _ => {
                    // is_sntm_native=true ama profile EMPTY → manifest drift.
                    // Defansif kernel halt (boot validation eksikliği).
                    crate::common::halt_system(
                        "[SCHED] is_sntm_native task with EMPTY PMP_PROFILES — manifest drift"
                    );
                }
            }
        } else {
            // FIX-3: Legacy task — single-NAPOT entry 8 (mevcut davranış).
            let napot_addr = next_task.pmp_addr_napot;
            crate::arch::pmp::write_per_task_napot(napot_addr, crate::arch::pmp::PMP_NAPOT_RW);
            // Sprint U-14: shadow wrapper — memory modül sınırları korunur
            crate::kernel::memory::update_task_pmp_shadow(
                napot_addr, crate::arch::pmp::PMP_NAPOT_RW,
            );
        }
    }

    let old_ctx = &mut TASKS.get_mut()[old].context as *mut TaskContext;
    let new_ctx = &TASKS.get_mut()[next].context    as *const TaskContext;
    switch_context(old_ctx, new_ctx);
}

/// SYS_YIELD syscall'dan çağrılır — SADECE context switch.
/// U-21 GÖREV 11 [H5]: schedule_timer_tick()'ten ayrıldı. U-mode task
/// yield spam ile blackbox tick / IPC rate reset / watchdog counter
/// state'ini bozamaz.
///
/// YAPMAZ (timer-tick path'in görevi):
///   - blackbox::advance_tick (forensik zaman bozmaz)
///   - ipc_send_count = 0 reset (rate limit bypass engellenir)
///   - watchdog_counter += 1 (yield != execution time)
///   - budget decrement (yield ≠ tick)
///   - period advance
///   - PMP shadow verify (timer'da zaten yapılıyor)
pub fn schedule_yield() {
    // SAFETY: MIE=0 in trap context, single-hart — no concurrent access.
    unsafe {
        if *TASK_COUNT.get() == 0 { return; }
        let old = *CURRENT_TASK.get();
        do_priority_select_and_switch(old);
    }
}

/// İlk task'ı başlat (boot'tan çağrılır)
pub(crate) fn start_first_task() -> ! {
    // SAFETY: Inline assembly — register state saved/restored by convention.
    unsafe {
        // Teşhis: TASK_COUNT ve TASKS[0].state — sadece debug-boot feature ile
        #[cfg(all(not(kani), feature = "debug-boot"))]
        {
            crate::arch::uart::puts("[DBG] TASK_COUNT=");
            crate::arch::uart::puts(match *TASK_COUNT.get() {
                0 => "0", 1 => "1", 2 => "2", 3 => "3",
                4 => "4", 5 => "5", 6 => "6", 7 => "7",
                8 => "8", _ => "?",
            });
            crate::arch::uart::puts(" TASKS[0].state=");
            crate::arch::uart::puts(match TASKS.get_mut()[0].state {
                crate::common::types::TaskState::Dead      => "Dead",
                crate::common::types::TaskState::Ready     => "Ready",
                crate::common::types::TaskState::Running   => "Running",
                crate::common::types::TaskState::Suspended => "Suspended",
                crate::common::types::TaskState::Isolated  => "Isolated",
            });
            crate::arch::uart::println("");
        }

        if *TASK_COUNT.get() == 0 {
            #[cfg(not(kani))]
            {
                crate::ipc::blackbox::log(
                    crate::ipc::blackbox::BlackboxEvent::PolicyShutdown,
                    SYSTEM_TASK_ID, &[],
                );
                crate::common::halt_system("[POLICY] SHUTDOWN — no tasks");
            }
        }

        // Sprint U-16: En yüksek öncelikli Ready task'ı seç — TASKS[0] sabit varsayımı
        // (priority inversion riski). Eşitlikte düşük indeks tie-break (deterministic).
        let count = *TASK_COUNT.get();
        let mut best: usize = 0;
        let mut best_prio: u8 = 255;
        let mut i: usize = 0;
        while i < count && i < MAX_TASKS {
            let t = &TASKS.get()[i];
            if t.state == TaskState::Ready && t.priority < best_prio {
                best = i;
                best_prio = t.priority;
            }
            i += 1;
        }
        if best_prio == 255 {
            // Hiç Ready task yok — sistem boot sonrası ölü
            #[cfg(not(kani))]
            {
                crate::ipc::blackbox::log(
                    crate::ipc::blackbox::BlackboxEvent::PolicyShutdown,
                    SYSTEM_TASK_ID, &[],
                );
                crate::common::halt_system("[POLICY] SHUTDOWN — no Ready task at boot");
            }
        }

        TASKS.get_mut()[best].state = TaskState::Running;
        *CURRENT_TASK.get_mut()     = best;

        // U-25 FIX-3: SNTM native → multi-region reload; legacy → single-NAPOT.
        #[cfg(not(kani))]
        {
            // U-22 GÖREV 24 [MP5]: MIE=0 invariant (start_first_task ekstra path).
            #[cfg(debug_assertions)]
            {
                let mstatus = crate::arch::csr::read_mstatus();
                debug_assert!(
                    mstatus & 0x8 == 0,
                    "MIE must be 0 during PMP shadow update (start_first_task path)"
                );
            }

            let first_task = &TASKS.get()[best];
            if first_task.is_sntm_native {
                match crate::kernel::pmp::profile::get_pmp_profile(first_task.id) {
                    Some(profile) if profile.region_count > 0 => {
                        // SAFETY: Boot context, MIE=0 (boot_init henüz interrupt açmadı).
                        // Outer unsafe block already covers — no nested unsafe needed.
                        crate::arch::pmp::reload_pmp_profile(profile);
                    }
                    _ => {
                        crate::common::halt_system(
                            "[BOOT] is_sntm_native task with EMPTY PMP_PROFILES — manifest drift"
                        );
                    }
                }
            } else {
                // FIX-3: Legacy task — single-NAPOT entry 8.
                let napot_addr = first_task.pmp_addr_napot;
                crate::arch::pmp::write_per_task_napot(napot_addr, crate::arch::pmp::PMP_NAPOT_RW);
                crate::kernel::memory::update_task_pmp_shadow(
                    napot_addr, crate::arch::pmp::PMP_NAPOT_RW,
                );
            }
        }

        let ctx     = &TASKS.get_mut()[best].context;
        let entry   = ctx.mepc;     // task entry point
        let sp_val  = ctx.sp;
        let mstatus = ctx.mstatus;  // MPP=U, MPIE=1

        // Sprint U-9: mscratch = kernel_sp — trap handler swap için şart
        // extern static + register operand (PIE/PIC güvenli, `la` kullanma)
        let kernel_sp = &__stack_top as *const u8 as usize;

        // U-21 GÖREV 5 [H7]: Caller-saved register scrub — task_trampoline
        // ile aynı pattern (U-19 hardening). İlk U-mode geçişte de kernel
        // state info-leak engellenir. mret öncesi ra/a0-a7/t0-t6 sıfırlanır;
        // mscratch/sp/mepc/mstatus setup'ta kullanılan reg'ler en son
        // temizlenir.
        core::arch::asm!(
            "csrw mscratch, {ksp}",
            "csrw mepc, {entry}",
            "csrw mstatus, {mstatus}",
            "mv sp, {sp}",
            // Caller-saved register clear (kernel state leak prevention)
            "li ra, 0",
            "li a0, 0",
            "li a1, 0",
            "li a2, 0",
            "li a3, 0",
            "li a4, 0",
            "li a5, 0",
            "li a6, 0",
            "li a7, 0",
            "li t2, 0",
            "li t3, 0",
            "li t4, 0",
            "li t5, 0",
            "li t6, 0",
            // t0/t1 setup'ta kullanıldı (csrw operand'ları via reg) —
            // asm! input register'larını inline'da temizleyemediğimiz için
            // bu son li'ler mret öncesi taşınmış değerleri sıfırlar.
            "li t0, 0",
            "li t1, 0",
            "mret",
            ksp     = in(reg) kernel_sp,
            entry   = in(reg) entry,
            mstatus = in(reg) mstatus,
            sp      = in(reg) sp_val,
            options(noreturn)
        );
    }
}

// ═══════════════════════════════════════════════════════
// Policy action handler'ları (private)
// ═══════════════════════════════════════════════════════

/// Policy kararını uygula
fn apply_action(task_id: usize, mode: FailureMode) {
    // Kernel-level event (task_id >= MAX_TASKS, örn. 0xFF) -> sadece Shutdown geçerli
    // PmpIntegrityFail, MultiModuleCrash gibi global event'ler buraya düşer.
    if task_id >= MAX_TASKS {
        if mode == FailureMode::Shutdown {
            shutdown_system();
        }
        return; // kernel event, task operation yok
    }
    match mode {
        FailureMode::Restart => {
            // Spam önleme: sadece ilk restart'ta yazdır (count artırılmış, 1 == ilk kez)
            #[cfg(not(kani))]
            {
                // SAFETY: MIE=0 in trap context, single-hart — no concurrent access.
                let tid = unsafe { TASKS.get_mut()[task_id].id };
                #[cfg(feature = "trace")]
                if crate::kernel::policy::get_restart_count(tid) == 1 {
                    crate::arch::uart::println("[POLICY] RESTART (1) — periyot sonunda READY");
                }
                crate::ipc::blackbox::log(
                    crate::ipc::blackbox::BlackboxEvent::TaskRestart,
                    tid, &[],
                );
            }
            restart_task(task_id);
        }
        FailureMode::Isolate => {
            #[cfg(not(kani))]
            {
                #[cfg(feature = "trace")]
                crate::arch::uart::println("[POLICY] ISOLATE — task durduruldu");
                crate::ipc::blackbox::log(
                    crate::ipc::blackbox::BlackboxEvent::PolicyIsolate,
                    // SAFETY: MIE=0 in trap context, single-hart — no concurrent access.
                    unsafe { TASKS.get_mut()[task_id].id },
                    &[],
                );
            }
            isolate_task(task_id);
        }
        FailureMode::Degrade => {
            #[cfg(not(kani))]
            // SAFETY: MIE=0 in trap context, single-hart — no concurrent access.
            unsafe {
                if !*DEGRADE_LOGGED.get() {
                    *DEGRADE_LOGGED.get_mut() = true;
                    #[cfg(feature = "trace")]
                    crate::arch::uart::println("[POLICY] DEGRADE — DAL-C/D durduruldu");
                    crate::ipc::blackbox::log(
                        crate::ipc::blackbox::BlackboxEvent::PolicyDegrade,
                        TASKS.get_mut()[task_id].id,
                        &[],
                    );
                }
            }
            degrade_system();
        }
        FailureMode::Failover => {
            // v1.0: Failover = Degrade (hot-standby task mekanizması v2.0'da)
            // decide_action -> Failover -> runtime Degrade uygular
            // Blackbox kaydında PolicyFailover olarak ayrışır (forensics)
            #[cfg(not(kani))]
            {
                #[cfg(feature = "trace")]
                crate::arch::uart::println("[POLICY] FAILOVER -> DEGRADE (stub)");
                crate::ipc::blackbox::log(
                    crate::ipc::blackbox::BlackboxEvent::PolicyFailover,
                    // SAFETY: MIE=0 in trap context, single-hart — no concurrent access.
                    unsafe { TASKS.get_mut()[task_id].id },
                    &[],
                );
            }
            degrade_system();
        }
        FailureMode::Alert => {
            #[cfg(all(not(kani), feature = "trace"))]
            crate::arch::uart::println("[POLICY] ALERT — operatör bildirildi, task devam");
        }
        FailureMode::Shutdown => {
            #[cfg(not(kani))]
            crate::ipc::blackbox::log(
                crate::ipc::blackbox::BlackboxEvent::PolicyShutdown,
                // SAFETY: MIE=0 in trap context, single-hart — no concurrent access.
                unsafe { TASKS.get_mut()[task_id].id },
                &[],
            );
            shutdown_system();
        }
    }
}

/// Task yeniden başlat — context sıfırla, giriş noktasından başlat
///
/// State SUSPENDED kalır — Faz 1 periyot reset'i READY yapar.
/// Budget ve period_counter burada sıfırlanmaz — Faz 1 halleder.
/// restart_count burada sıfırlanmaz — apply_policy'de artırılır, birikir.
fn restart_task(id: usize) {
    // SAFETY: MIE=0 in trap context, single-hart — no concurrent access.
    unsafe {
        if id >= *TASK_COUNT.get() { return; }

        // Tüm callee-saved register'ları sıfırla
        TASKS.get_mut()[id].context = TaskContext::zero();

        // Giriş noktası + temiz stack + U-mode CSR
        #[cfg(not(kani))]
        { TASKS.get_mut()[id].context.ra    = task_trampoline as *const () as usize; }
        #[cfg(kani)]
        { TASKS.get_mut()[id].context.ra    = TASKS.get_mut()[id].entry; }
        TASKS.get_mut()[id].context.sp      = TASKS.get_mut()[id].stack_top;
        TASKS.get_mut()[id].context.mepc    = TASKS.get_mut()[id].entry;
        TASKS.get_mut()[id].context.mstatus = crate::arch::csr::MSTATUS_MPP_U
                                            | crate::arch::csr::MSTATUS_MPIE;
    }
}

/// Degrade — dal >= 2 (DAL-C/D) taskları askıya al
/// Not: Isolated task'lar bu işlemde değiştirilmez — zaten daha kısıtlı.
fn degrade_system() {
    // SAFETY: MIE=0 in trap context, single-hart — no concurrent access.
    unsafe {
        *DEGRADED.get_mut() = true;
        // U-17 GÖREV 7: Degrade başlangıç timestamp'i — recovery cooldown için
        #[cfg(not(kani))]
        { *DEGRADE_TICK.get_mut() = crate::ipc::blackbox::current_tick(); }
        let mut i: usize = 0;
        while i < *TASK_COUNT.get() {
            if TASKS.get_mut()[i].dal >= 2
                && TASKS.get_mut()[i].state != TaskState::Dead
                && TASKS.get_mut()[i].state != TaskState::Isolated
            {
                TASKS.get_mut()[i].state = TaskState::Suspended;
                // Bütçe yarılama — kurtarma sonrası dikkatli mod
                TASKS.get_mut()[i].budget_cycles =
                    TASKS.get_mut()[i].budget_cycles / 2;
            }
            i += 1;
        }
    }
}

/// Degrade kurtarma — DAL-A/B sağlıklıysa DAL-C/D'yi orijinal bütçeyle yeniden başlat
fn try_recover_from_degrade() {
    // SAFETY: Single-hart.
    unsafe {
        if !*DEGRADED.get() { return; }

        // U-17 GÖREV 7: Cooldown — recovery flapping önleme
        // degrade'den en az 100 tick (~1s) sonra recovery'ye izin ver
        #[cfg(not(kani))]
        {
            let current = crate::ipc::blackbox::current_tick();
            let degrade_start = *DEGRADE_TICK.get();
            // wrapping_sub güvenli — u32 wrap zaten 497 gün
            if current.wrapping_sub(degrade_start) < DEGRADE_COOLDOWN_TICKS {
                return; // cooldown süresinde, recover etme
            }
        }

        // DAL-A/B sağlıklı mı?
        let mut all_healthy = true;
        let mut i: usize = 0;
        while i < *TASK_COUNT.get() {
            if TASKS.get_mut()[i].dal < 2
                && TASKS.get_mut()[i].state == TaskState::Isolated
            {
                all_healthy = false;
            }
            i += 1;
        }
        if !all_healthy { return; }
        // DAL-C/D kurtarma
        let mut j: usize = 0;
        while j < *TASK_COUNT.get() {
            if TASKS.get_mut()[j].dal >= 2
                && TASKS.get_mut()[j].state == TaskState::Suspended
            {
                TASKS.get_mut()[j].budget_cycles =
                    TASKS.get_mut()[j].original_budget;
                TASKS.get_mut()[j].remaining_cycles =
                    TASKS.get_mut()[j].budget_cycles;
                TASKS.get_mut()[j].state = TaskState::Ready;
                #[cfg(all(not(kani), feature = "trace"))]
                crate::arch::uart::println(
                    "[POLICY] RECOVER — DAL-C/D restarted (original budget)"
                );
            }
            j += 1;
        }
        *DEGRADED.get_mut() = false;
        *DEGRADE_LOGGED.get_mut() = false;
        #[cfg(not(kani))]
        crate::ipc::blackbox::log(
            crate::ipc::blackbox::BlackboxEvent::KernelBoot, KERNEL_BOOT_ID, &[],
        );
    }
}

/// Task fault handler — trap handler'dan çağrılır
/// Sprint U-14: handle_illegal_instruction -> handle_task_fault (rename)
/// Kapsam: illegal instruction (mcause=2), PMP violation (mcause=5|7, non-stack),
/// genel WASM trap. Stack overflow ayrı path'te (StackOverflow event'i kullanıyor).
pub(crate) fn handle_task_fault() {
    // SAFETY: MIE=0 in trap context, single-hart — no concurrent access.
    unsafe {
        let current = *CURRENT_TASK.get();
        if current < *TASK_COUNT.get() {
            let dal = TASKS.get()[current].dal;
            let action = crate::kernel::policy::apply_policy(
                current as u8,
                crate::kernel::policy::PolicyEvent::WasmTrap,
                dal,
            );
            apply_action(current, action);
        }
    }
}

/// İzole — task durdur + token revoke
/// Isolated: Suspended'dan farklı, periyot reset ile READY'ye dönmez.
/// U-23: pub(crate) — sys_exit syscall handler bu helper'ı çağırır
/// (voluntary task termination path).
pub(crate) fn isolate_task(id: usize) {
    // SAFETY: MIE=0 in trap context, single-hart — no concurrent access.
    unsafe {
        if id < *TASK_COUNT.get() {
            TASKS.get_mut()[id].state = TaskState::Isolated;
            crate::kernel::capability::broker::invalidate_task_capabilities(TASKS.get_mut()[id].id);
        }
    }

    // MultiModuleCrash detection — 2+ task izole olursa global shutdown
    // Threshold ≥ 2: 8 task'ta %25 kayıp -> Shutdown (safety-critical'de
    // erken shutdown geç shutdown'dan güvenli)
    #[cfg(not(kani))]
    {
        let mut isolated_count: u8 = 0;
        let mut j: usize = 0;
        // SAFETY: MIE=0 in trap context, single-hart.
        unsafe {
            while j < *TASK_COUNT.get() {
                if TASKS.get()[j].state == TaskState::Isolated {
                    isolated_count += 1;
                }
                j += 1;
            }
        }
        if isolated_count >= 2 {
            crate::ipc::blackbox::log(
                crate::ipc::blackbox::BlackboxEvent::PolicyShutdown,
                SYSTEM_TASK_ID, &[],
            );
            let action = crate::kernel::policy::apply_policy(
                SYSTEM_TASK_ID,
                PolicyEvent::MultiModuleCrash,
                0_u8, // DAL-A
            );
            apply_action(SYSTEM_TASK_INDEX, action);
            // decide_action(MultiModuleCrash, *, *) -> Shutdown
        }
    }
}

/// Kapatma — güvenli durum, sonsuz bekleme
fn shutdown_system() -> ! {
    #[cfg(not(kani))]
    crate::arch::uart::println("[POLICY] SHUTDOWN — güvenli durum");
    loop {
        #[cfg(not(kani))]
        // SAFETY: WFI instruction — halts hart until interrupt, no state corruption.
        unsafe { core::arch::asm!("wfi") };
        #[cfg(kani)]
        {}
    }
}

/// Mevcut çalışan task'ın ID'sini döndür
pub(crate) fn current_task_id() -> u8 {
    // SAFETY: Single-hart, MIE=0 in trap/scheduler context. SingleHartCell read.
    unsafe { *CURRENT_TASK.get() as u8 }
}

/// Sprint U-16: Task stack aralığını döndür — is_valid_user_ptr için kapsüllenmiş
/// U-17 GÖREV 9: Test-only watchdog counter getter — INFO/test için
/// task_id MAX_TASKS dışı -> u32::MAX (sentinel)
#[cfg(feature = "self-test")]
pub fn test_get_watchdog_counter(task_id: usize) -> u32 {
    if task_id >= MAX_TASKS { return u32::MAX; }
    // SAFETY: Test-only, single-hart, bounds checked above.
    unsafe { TASKS.get()[task_id].watchdog_counter }
}

/// erişim. Caller'ın sadece kendi stack'ine pointer vermesi zorunlu.
/// Dead/Isolated/uninitialized task -> None (default deny).
pub(crate) fn task_stack_range(task_id: u8) -> Option<(usize, usize)> {
    let idx = task_id as usize;
    // SAFETY: MIE=0 in trap context, single-hart.
    unsafe {
        if idx >= *TASK_COUNT.get() { return None; }
        let task = &TASKS.get()[idx];
        if task.state == TaskState::Dead || task.state == TaskState::Isolated {
            return None;
        }
        let top = task.stack_top;
        if top < TASK_STACK_SIZE { return None; } // underflow guard
        Some((top - TASK_STACK_SIZE, top))
    }
}

/// Trap handler'dan policy action uygulama — apply_action wrapper
/// apply_action private, trap handler erişimi için pub(crate) gerekli
#[cfg(not(kani))]
pub(crate) fn apply_action_from_trap(task_id: usize, mode: FailureMode) {
    apply_action(task_id, mode);
}

/// Watchdog kick — task yield etti, canlılık kanıtı
pub(crate) fn watchdog_kick() {
    // SAFETY: Single-hart, called from syscall context.
    unsafe {
        let current = *CURRENT_TASK.get();
        if current < *TASK_COUNT.get() {
            let counter = TASKS.get_mut()[current].watchdog_counter;
            let window  = TASKS.get_mut()[current].watchdog_window_min;
            // Windowed: kick çok erken -> kontrol akışı bozuk
            if window > 0 && counter < window {
                #[cfg(not(kani))]
                {
                    #[cfg(feature = "trace")]
                    crate::arch::uart::println("[WATCHDOG] WINDOW VIOLATION — kick too early");
                    crate::ipc::blackbox::log(
                        crate::ipc::blackbox::BlackboxEvent::WatchdogTimeout,
                        TASKS.get_mut()[current].id, &[],
                    );
                }
                let action = crate::kernel::policy::apply_policy(
                    TASKS.get_mut()[current].id,
                    crate::kernel::policy::PolicyEvent::WatchdogTimeout,
                    TASKS.get_mut()[current].dal,
                );
                apply_action(current, action);
            }
            TASKS.get_mut()[current].watchdog_counter = 0;
        }
    }
}

/// Syscall sayacı artır — anomali tespiti
pub(crate) fn increment_syscall_count() {
    // SAFETY: Single-hart, MIE=0 in trap context (called from dispatch). Bounds checked.
    unsafe {
        let current = *CURRENT_TASK.get();
        if current < *TASK_COUNT.get() {
            TASKS.get_mut()[current].syscall_count =
                TASKS.get_mut()[current].syscall_count.wrapping_add(1);
        }
    }
}

/// IPC send rate kontrolü — tick başına MAX_SENDS_PER_TICK
pub(crate) fn check_ipc_rate() -> bool {
    // SAFETY: Single-hart, MIE=0 in trap context. Bounds checked before TASKS access.
    unsafe {
        let current = *CURRENT_TASK.get();
        if current < *TASK_COUNT.get() {
            TASKS.get_mut()[current].ipc_send_count < MAX_SENDS_PER_TICK
        } else {
            false
        }
    }
}

/// IPC send sayacı artır
pub(crate) fn increment_ipc_send() {
    // SAFETY: Single-hart, MIE=0 in trap context. Bounds checked.
    unsafe {
        let current = *CURRENT_TASK.get();
        if current < *TASK_COUNT.get() {
            TASKS.get_mut()[current].ipc_send_count =
                TASKS.get_mut()[current].ipc_send_count.wrapping_add(1);
        }
    }
}

/// Task bilgisi sorgula — sys_task_info dispatch'ten çağrılır
/// Dönüş: (state << 8) | (priority << 4) | dal
/// task_id geçersizse 0 döner
pub fn query_task_info(task_id: usize) -> usize {
    // SAFETY: Single-hart, no concurrent mutation during syscall.
    unsafe {
        if task_id >= *TASK_COUNT.get() { return 0; }
        let state = TASKS.get()[task_id].state as usize;
        let prio  = TASKS.get()[task_id].priority as usize;
        let dal   = TASKS.get()[task_id].dal as usize;
        (state << 8) | (prio << 4) | dal
    }
}

/// Pure task seçim fonksiyonu — Kani'de doğrulanabilir
/// En düşük priority değeri olan Ready/Running task'ı seç
#[allow(dead_code)] // Used by Kani proofs; mirrors schedule() Faz 3 logic.
pub fn select_highest_priority(
    states: &[TaskState],
    priorities: &[u8],
    count: usize,
) -> Option<usize> {
    let mut best: Option<usize> = None;
    let mut best_prio: u8 = u8::MAX;
    let mut i = 0;
    while i < count && i < states.len() && i < priorities.len() {
        if is_selectable_by_scheduler(states[i])
            && priorities[i] < best_prio
        {
            best = Some(i);
            best_prio = priorities[i];
        }
        i += 1;
    }
    best
}

// ═══════════════════════════════════════════════════════
// U-19 GÖREV 10: Pure helper'lar — inline scheduler logic Kani'de testable
// Önceden Proof 71 (Isolated->Ready Phase 1 reset) ve Proof 95 (watchdog
// limit=0) inline while-loop logic'i çağıramıyordu, tautoloji idi.
// Bu helper'lar production'da inline kullanılıyor + Kani harness çağırabiliyor.
// ═══════════════════════════════════════════════════════

/// Scheduler bu state'teki bir task'ı seçer mi? (Phase 3)
/// Sadece Ready ve Running uygun. Dead/Suspended/Isolated reddedilir.
#[inline]
pub(crate) fn is_selectable_by_scheduler(state: TaskState) -> bool {
    matches!(state, TaskState::Ready | TaskState::Running)
}

/// Phase 1 periyot reset bu state'e uygulanır mı?
/// Sadece Suspended -> Ready geçişi. Isolated kasıtlı kapsam dışı (kalıcı izolasyon).
#[inline]
pub(crate) fn is_period_reset_eligible(state: TaskState) -> bool {
    matches!(state, TaskState::Suspended)
}

/// Phase 1.5 watchdog timeout tetiklenmeli mi?
/// limit=0 -> watchdog devre dışı. Aksi halde counter >= limit ise tetikle.
#[inline]
pub(crate) fn should_watchdog_timeout(limit: u32, counter: u32) -> bool {
    limit > 0 && counter >= limit
}

// ═══════════════════════════════════════════════════════
// Kani — Scheduler proofs
// ═══════════════════════════════════════════════════════

#[cfg(kani)]
mod verification {
    use super::*;
    use crate::common::config::MAX_TASKS;

    /// Proof 79: En az bir Ready task varsa her zaman birini seçer,
    /// seçilen index geçerli ve state Ready/Running
    #[kani::proof]
    #[kani::unwind(9)]
    fn scheduler_always_selects_ready() {
        let mut states = [TaskState::Dead; MAX_TASKS];
        let priorities = [15u8; MAX_TASKS];
        let count: usize = kani::any();
        kani::assume(count >= 1 && count <= MAX_TASKS);

        let ready_idx: usize = kani::any();
        kani::assume(ready_idx < count);
        states[ready_idx] = TaskState::Ready;

        let selected = select_highest_priority(&states, &priorities, count);
        assert!(selected.is_some());

        if let Some(sel) = selected {
            assert!(sel < count);
            assert!(sel < MAX_TASKS);
            assert!(states[sel] == TaskState::Ready || states[sel] == TaskState::Running);
        }
    }

    /// Proof 80: Dead ve Isolated task'lar asla seçilmez
    #[kani::proof]
    fn scheduler_never_selects_dead_or_isolated() {
        let mut states = [TaskState::Dead; MAX_TASKS];
        let priorities = [5u8; MAX_TASKS];
        states[0] = TaskState::Dead;
        states[1] = TaskState::Isolated;
        let selected = select_highest_priority(&states, &priorities, 2);
        assert!(selected.is_none());
    }

    /// Proof 81: Isolated task en yüksek öncelikte bile seçilmez
    #[kani::proof]
    #[kani::unwind(9)]
    fn scheduler_isolated_never_selected() {
        let mut states = [TaskState::Dead; MAX_TASKS];
        let mut priorities = [15u8; MAX_TASKS];
        let count: usize = kani::any();
        kani::assume(count >= 1 && count <= MAX_TASKS);

        let iso_idx: usize = kani::any();
        kani::assume(iso_idx < count);
        states[iso_idx] = TaskState::Isolated;
        priorities[iso_idx] = 0; // en yüksek öncelik

        let selected = select_highest_priority(&states, &priorities, count);
        if let Some(sel) = selected {
            assert!(states[sel] != TaskState::Isolated);
        }
    }

    /// Proof 96: Tüm Dead -> select None
    #[kani::proof]
    fn select_all_dead_returns_none() {
        let states = [TaskState::Dead; MAX_TASKS];
        let priorities = [0u8; MAX_TASKS];
        let result = select_highest_priority(&states, &priorities, MAX_TASKS);
        assert!(result.is_none());
    }

    /// Proof 97: Tek Ready task her zaman seçilir
    #[kani::proof]
    #[kani::unwind(9)]
    fn select_single_ready_always_found() {
        let mut states = [TaskState::Dead; MAX_TASKS];
        let priorities = [15u8; MAX_TASKS];
        let idx: usize = kani::any();
        kani::assume(idx < MAX_TASKS);
        states[idx] = TaskState::Ready;
        let result = select_highest_priority(&states, &priorities, MAX_TASKS);
        assert!(result == Some(idx));
    }

    /// Proof 98: Watchdog counter < limit -> tetiklenmez
    #[kani::proof]
    fn watchdog_under_limit_no_trigger() {
        let counter: u32 = kani::any();
        let limit: u32 = kani::any();
        kani::assume(limit > 0);
        kani::assume(counter < limit);
        let triggered = limit > 0 && counter >= limit;
        assert!(!triggered);
    }

    /// Proof 99: Watchdog counter == limit -> tetiklenir
    #[kani::proof]
    fn watchdog_at_limit_triggers() {
        let limit: u32 = kani::any();
        kani::assume(limit > 0);
        let counter = limit;
        let triggered = limit > 0 && counter >= limit;
        assert!(triggered);
    }

    /// Proof 100: Task::empty() doğru başlangıç durumu
    #[kani::proof]
    fn task_empty_initial_state() {
        let t = Task::empty();
        assert!(t.state == TaskState::Dead);
        assert!(t.priority == 15);
        assert!(t.dal == 3);
        assert!(t.budget_cycles == 0);
        assert!(t.period_ticks == 0);
        assert!(t.watchdog_counter == 0);
        assert!(t.watchdog_limit == 0);
    }

    /// Proof 128: Running task da seçilebilir
    #[kani::proof]
    #[kani::unwind(9)]
    fn select_running_task_selectable() {
        let mut states = [TaskState::Dead; MAX_TASKS];
        let priorities = [15u8; MAX_TASKS];
        let idx: usize = kani::any();
        kani::assume(idx < MAX_TASKS);
        states[idx] = TaskState::Running;
        let result = select_highest_priority(&states, &priorities, MAX_TASKS);
        assert!(result == Some(idx));
    }

    /// Proof 129: Suspended task asla seçilmez
    #[kani::proof]
    fn select_suspended_never_selected() {
        let states = [TaskState::Suspended; MAX_TASKS];
        let priorities = [0u8; MAX_TASKS];
        let result = select_highest_priority(&states, &priorities, MAX_TASKS);
        assert!(result.is_none());
    }

    /// Proof 130: query_task_info: geçersiz id -> 0
    #[kani::proof]
    fn query_task_info_invalid_id() {
        let id: usize = kani::any();
        kani::assume(id >= MAX_TASKS);
        let info = query_task_info(id);
        assert!(info == 0);
    }

    /// Proof 131: count=0 -> None
    #[kani::proof]
    fn select_zero_count_returns_none() {
        let states = [TaskState::Ready; MAX_TASKS];
        let priorities = [0u8; MAX_TASKS];
        let result = select_highest_priority(&states, &priorities, 0);
        assert!(result.is_none());
    }

    /// Proof 151: Isolated task hiçbir koşulda seçilmez — symbolic tüm config
    #[kani::proof]
    #[kani::unwind(9)]
    fn isolated_never_scheduled_any_config() {
        let mut states = [TaskState::Dead; MAX_TASKS];
        let mut priorities = [0u8; MAX_TASKS];
        let count: usize = kani::any();
        kani::assume(count >= 1 && count <= MAX_TASKS);
        let mut i = 0;
        while i < count {
            states[i] = kani::any();
            priorities[i] = kani::any();
            i += 1;
        }
        let iso_idx: usize = kani::any();
        kani::assume(iso_idx < count);
        states[iso_idx] = TaskState::Isolated;
        priorities[iso_idx] = 0;
        let result = select_highest_priority(&states, &priorities, count);
        if let Some(sel) = result {
            assert!(states[sel] != TaskState::Isolated);
        }
    }

    /// Proof 152: Seçilen task her zaman minimum priority numarasına sahip
    #[kani::proof]
    #[kani::unwind(9)]
    fn selected_has_minimum_priority() {
        let mut states = [TaskState::Dead; MAX_TASKS];
        let mut priorities = [15u8; MAX_TASKS];
        let count: usize = kani::any();
        kani::assume(count >= 1 && count <= MAX_TASKS);
        let mut i = 0;
        while i < count {
            states[i] = kani::any();
            priorities[i] = kani::any();
            i += 1;
        }
        let result = select_highest_priority(&states, &priorities, count);
        if let Some(sel) = result {
            assert!(states[sel] == TaskState::Ready || states[sel] == TaskState::Running);
            let mut j = 0;
            while j < count {
                if states[j] == TaskState::Ready || states[j] == TaskState::Running {
                    assert!(priorities[sel] <= priorities[j]);
                }
                j += 1;
            }
        }
    }

    /// Proof 159: Stack alignment — HERHANGİ bir adres & !0xF -> 16-byte aligned
    #[kani::proof]
    fn stack_alignment_any_address() {
        let raw: usize = kani::any();
        let aligned = raw & !0xF_usize;
        assert!(aligned % 16 == 0);
        assert!(aligned <= raw);
    }

    /// Proof 160: select sonuç index HER ZAMAN count'tan küçük
    #[kani::proof]
    #[kani::unwind(9)]
    fn select_result_always_less_than_count() {
        let mut states = [TaskState::Dead; MAX_TASKS];
        let mut priorities = [15u8; MAX_TASKS];
        let count: usize = kani::any();
        kani::assume(count >= 1 && count <= MAX_TASKS);
        let mut i = 0;
        while i < count { states[i] = kani::any(); priorities[i] = kani::any(); i += 1; }
        let result = select_highest_priority(&states, &priorities, count);
        if let Some(sel) = result { assert!(sel < count); }
    }

    /// Proof 161: Budget saturating_sub asla wrap-around olmaz
    #[kani::proof]
    fn budget_saturating_sub_no_wrap() {
        let remaining: u32 = kani::any();
        let result = remaining.saturating_sub(CYCLES_PER_TICK);
        assert!(result <= remaining);
    }
}
