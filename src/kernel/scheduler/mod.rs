//! Preemptive fixed-priority scheduler with per-task budget and period enforcement.
// Sipahi — Scheduler (Sprint 10)
// Fixed-Priority Preemptive + Budget + Deadline
//
// Sprint 4:  Round-robin
// Sprint 10: priority (0-15), budget_cycles, period_ticks, policy engine entegrasyonu
//
// Her tick:
//   1. Tüm task'lar için period ilerlet → süre dolduysa bütçe sıfırla + READY
//   2. Mevcut task bütçesini düş → 0 ise SUSPENDED + policy
//   3. En yüksek öncelikli Ready task'ı seç (düşük sayı = yüksek öncelik)
//   4. Context switch
//
// WCET: ≤0.8μs (doküman §SCHEDULER)

use crate::common::config::{MAX_TASKS, TASK_STACK_SIZE, CYCLES_PER_TICK};
use crate::common::sync::SingleHartCell;
use crate::common::types::TaskState;
use crate::kernel::policy::{FailureMode, PolicyEvent};

// ═══════════════════════════════════════════════════════
// TaskContext: callee-saved registers
// ═══════════════════════════════════════════════════════

/// Callee-saved register'lar — context.S ile eşleşmeli
/// 14 register × 8 byte = 112 byte
#[repr(C)]
pub struct TaskContext {
    pub ra:  usize,  // Return address
    pub sp:  usize,  // Stack pointer
    pub s0:  usize,  // Saved registers
    pub s1:  usize,
    pub s2:  usize,
    pub s3:  usize,
    pub s4:  usize,
    pub s5:  usize,
    pub s6:  usize,
    pub s7:  usize,
    pub s8:  usize,
    pub s9:  usize,
    pub s10: usize,
    pub s11: usize,
}

impl TaskContext {
    pub const fn zero() -> Self {
        TaskContext {
            ra: 0, sp: 0,
            s0: 0, s1: 0, s2: 0, s3: 0,
            s4: 0, s5: 0, s6: 0, s7: 0,
            s8: 0, s9: 0, s10: 0, s11: 0,
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
        }
    }
}

// ═══════════════════════════════════════════════════════
// Statik alanlar
// ═══════════════════════════════════════════════════════

/// Task stack'leri — statik, heap yok
static TASK_STACKS: SingleHartCell<[[u8; TASK_STACK_SIZE]; MAX_TASKS]> = SingleHartCell::new([[0u8; TASK_STACK_SIZE]; MAX_TASKS]);

/// Task dizisi — statik
static TASKS: SingleHartCell<[Task; MAX_TASKS]> = SingleHartCell::new([
    Task::empty(), Task::empty(), Task::empty(), Task::empty(),
    Task::empty(), Task::empty(), Task::empty(), Task::empty(),
]);

/// Aktif task sayısı
static TASK_COUNT: SingleHartCell<usize> = SingleHartCell::new(0);

/// DEGRADE mesajı bir kez yazdırılsın — spam önleme
static DEGRADE_LOGGED: SingleHartCell<bool> = SingleHartCell::new(false);

/// Şu anki çalışan task index'i
static CURRENT_TASK: SingleHartCell<usize> = SingleHartCell::new(0);

// context.S'deki switch_context fonksiyonu
extern "C" {
    fn switch_context(old: *mut TaskContext, new: *const TaskContext);
}

// ═══════════════════════════════════════════════════════
// Public API
// ═══════════════════════════════════════════════════════

/// Yeni task oluştur
/// priority: 0-15 (0=en yüksek, DAL-A=0-3, DAL-B=4-7, DAL-C=8-11, DAL-D=12-15)
/// dal: 0=A 1=B 2=C 3=D
/// budget_cycles: periyot başına CPU bütçesi (0 = sınırsız)
/// period_ticks:  periyot uzunluğu tick cinsinden (0 = periyotsuz)
pub fn create_task(
    entry: fn() -> !,
    priority: u8,
    dal: u8,
    budget_cycles: u32,
    period_ticks: u32,
) -> Option<u8> {
    // SAFETY: Single-hart system, interrupts disabled during boot — no concurrent access.
    unsafe {
        if *TASK_COUNT.get() >= MAX_TASKS {
            return None;
        }

        let id = *TASK_COUNT.get();

        // Stack tepesi — 16-byte hizalı (RISC-V ABI)
        let stack_top         = TASK_STACKS.get_mut()[id].as_ptr() as usize + TASK_STACK_SIZE;
        let stack_top_aligned = stack_top & !0xF;

        TASKS.get_mut()[id].id               = id as u8;
        TASKS.get_mut()[id].state            = TaskState::Ready;
        TASKS.get_mut()[id].context          = TaskContext::zero();
        TASKS.get_mut()[id].context.ra       = entry as usize;
        TASKS.get_mut()[id].context.sp       = stack_top_aligned;
        TASKS.get_mut()[id].entry            = entry as usize;
        TASKS.get_mut()[id].stack_top        = stack_top_aligned;
        TASKS.get_mut()[id].priority         = priority;
        TASKS.get_mut()[id].dal              = dal;
        TASKS.get_mut()[id].budget_cycles    = budget_cycles;
        TASKS.get_mut()[id].remaining_cycles = budget_cycles;
        TASKS.get_mut()[id].period_ticks     = period_ticks;
        TASKS.get_mut()[id].period_counter   = 0;

        *TASK_COUNT.get_mut() += 1;
        Some(id as u8)
    }
}

/// Scheduler tick — timer interrupt'tan çağrılır
/// WCET: ≤0.8μs @ 100MHz (doküman hedef)
pub fn schedule() {
    // SAFETY: Single-hart system, interrupts disabled during boot — no concurrent access.
    unsafe {
        if *TASK_COUNT.get() < 2 {
            return;
        }

        // Blackbox tick sayacını ilerlet (schedule() her çağrısında)
        #[cfg(not(kani))]
        crate::ipc::blackbox::advance_tick();

        let old = *CURRENT_TASK.get();

        // ─── Faz 1: Periyot ilerletme (tüm task'lar) ───
        // Periyot dolarsa: bütçe sıfırla, SUSPENDED → READY
        let mut i: usize = 0;
        while i < *TASK_COUNT.get() {
            if TASKS.get_mut()[i].period_ticks > 0 {
                TASKS.get_mut()[i].period_counter = TASKS.get_mut()[i].period_counter.wrapping_add(1);
                if TASKS.get_mut()[i].period_counter >= TASKS.get_mut()[i].period_ticks {
                    TASKS.get_mut()[i].period_counter   = 0;
                    TASKS.get_mut()[i].remaining_cycles = TASKS.get_mut()[i].budget_cycles;
                    if TASKS.get_mut()[i].state == TaskState::Suspended {
                        TASKS.get_mut()[i].state = TaskState::Ready;
                        // Not: Isolated task'lar bu blokta hiç eşleşmez —
                        // kasıtlı. Isolated → periyot reset ile READY yapılmaz.
                    }
                }
            }
            i += 1;
        }

        // ─── Faz 2: Bütçe düşümü + politika (mevcut task) ───
        // budget_cycles == 0 → sınırsız bütçe (bütçe izleme devre dışı)
        if TASKS.get_mut()[old].budget_cycles > 0 && TASKS.get_mut()[old].state == TaskState::Running {
            TASKS.get_mut()[old].remaining_cycles =
                TASKS.get_mut()[old].remaining_cycles.saturating_sub(CYCLES_PER_TICK);

            if TASKS.get_mut()[old].remaining_cycles == 0 {
                TASKS.get_mut()[old].state = TaskState::Suspended;

                // Blackbox: bütçe tükenmesi kaydı
                #[cfg(not(kani))]
                {
                    let ev = [TASKS.get_mut()[old].id, TASKS.get_mut()[old].dal];
                    crate::ipc::blackbox::log(
                        crate::ipc::blackbox::BlackboxEvent::BudgetExhausted,
                        TASKS.get_mut()[old].id,
                        &ev,
                    );
                }

                let action = crate::kernel::policy::apply_policy(
                    TASKS.get_mut()[old].id,
                    PolicyEvent::BudgetExhausted,
                    TASKS.get_mut()[old].dal,
                );
                apply_action(old, action);
            }
        }

        // ─── Faz 3: En yüksek öncelikli Ready task ───
        // Düşük priority sayısı = yüksek öncelik (DAL-A = 0-3)
        let mut next      = usize::MAX;
        let mut best_prio = u8::MAX;
        let mut j: usize  = 0;
        while j < *TASK_COUNT.get() {
            let s = TASKS.get_mut()[j].state;
            if (s == TaskState::Ready || s == TaskState::Running)
                && TASKS.get_mut()[j].priority < best_prio
            {
                best_prio = TASKS.get_mut()[j].priority;
                next      = j;
            }
            j += 1;
        }

        if next == usize::MAX {
            return; // Hiç Ready task yok
        }

        // Aynı task, hâlâ çalışıyor → context switch gerekmez
        if next == old && TASKS.get_mut()[old].state == TaskState::Running {
            return;
        }

        // ─── Faz 4: Context switch ───
        if TASKS.get_mut()[old].state == TaskState::Running {
            TASKS.get_mut()[old].state = TaskState::Ready;
        }
        TASKS.get_mut()[next].state = TaskState::Running;
        *CURRENT_TASK.get_mut()     = next;

        let old_ctx = &mut TASKS.get_mut()[old].context as *mut TaskContext;
        let new_ctx = &TASKS.get_mut()[next].context    as *const TaskContext;
        switch_context(old_ctx, new_ctx);
    }
}

/// İlk task'ı başlat (boot'tan çağrılır)
pub fn start_first_task() -> ! {
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
                crate::arch::uart::println("[POLICY] SHUTDOWN — no tasks");
                crate::ipc::blackbox::log(
                    crate::ipc::blackbox::BlackboxEvent::PolicyShutdown, 0xFF, &[],
                );
                loop { core::arch::asm!("wfi"); }
            }
        }

        TASKS.get_mut()[0].state = TaskState::Running;
        *CURRENT_TASK.get_mut()  = 0;

        let ctx   = &TASKS.get_mut()[0].context;
        let entry = ctx.ra;
        let sp    = ctx.sp;

        core::arch::asm!(
            "mv sp, {sp}",
            "jr {entry}",
            sp    = in(reg) sp,
            entry = in(reg) entry,
            options(noreturn)
        );
    }
}

// ═══════════════════════════════════════════════════════
// Policy action handler'ları (private)
// ═══════════════════════════════════════════════════════

/// Policy kararını uygula
fn apply_action(task_id: usize, mode: FailureMode) {
    match mode {
        FailureMode::Restart => {
            // Spam önleme: sadece ilk restart'ta yazdır (count artırılmış, 1 == ilk kez)
            #[cfg(not(kani))]
            {
                // SAFETY: Single-hart system, interrupts disabled during boot — no concurrent access.
                let tid = unsafe { TASKS.get_mut()[task_id].id };
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
                crate::arch::uart::println("[POLICY] ISOLATE — task durduruldu");
                crate::ipc::blackbox::log(
                    crate::ipc::blackbox::BlackboxEvent::PolicyIsolate,
                    // SAFETY: Single-hart system, interrupts disabled during boot — no concurrent access.
                    unsafe { TASKS.get_mut()[task_id].id },
                    &[],
                );
            }
            isolate_task(task_id);
        }
        FailureMode::Degrade => {
            #[cfg(not(kani))]
            // SAFETY: Single-hart system, interrupts disabled during boot — no concurrent access.
            unsafe {
                if !*DEGRADE_LOGGED.get() {
                    *DEGRADE_LOGGED.get_mut() = true;
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
            // v1.0 stub: yedek task mekanizması Sprint 12+
            #[cfg(not(kani))]
            {
                crate::arch::uart::println("[POLICY] FAILOVER (stub) → DEGRADE");
                crate::ipc::blackbox::log(
                    crate::ipc::blackbox::BlackboxEvent::PolicyFailover,
                    // SAFETY: Single-hart system, interrupts disabled during boot — no concurrent access.
                    unsafe { TASKS.get_mut()[task_id].id },
                    &[],
                );
            }
            degrade_system();
        }
        FailureMode::Alert => {
            #[cfg(not(kani))]
            crate::arch::uart::println("[POLICY] ALERT — operatör bildirildi, task devam");
        }
        FailureMode::Shutdown => {
            #[cfg(not(kani))]
            crate::ipc::blackbox::log(
                crate::ipc::blackbox::BlackboxEvent::PolicyShutdown,
                // SAFETY: Single-hart system, interrupts disabled during boot — no concurrent access.
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
    // SAFETY: Single-hart system, interrupts disabled during boot — no concurrent access.
    unsafe {
        if id >= *TASK_COUNT.get() { return; }

        // Tüm callee-saved register'ları sıfırla
        TASKS.get_mut()[id].context = TaskContext::zero();

        // Giriş noktası + temiz stack
        TASKS.get_mut()[id].context.ra = TASKS.get_mut()[id].entry;
        TASKS.get_mut()[id].context.sp = TASKS.get_mut()[id].stack_top;
    }
}

/// Degrade — dal >= 2 (DAL-C/D) taskları askıya al
/// Not: Isolated task'lar bu işlemde değiştirilmez — zaten daha kısıtlı.
fn degrade_system() {
    // SAFETY: Single-hart system, interrupts disabled during boot — no concurrent access.
    unsafe {
        let mut i: usize = 0;
        while i < *TASK_COUNT.get() {
            if TASKS.get_mut()[i].dal >= 2
                && TASKS.get_mut()[i].state != TaskState::Dead
                && TASKS.get_mut()[i].state != TaskState::Isolated
            {
                TASKS.get_mut()[i].state = TaskState::Suspended;
            }
            i += 1;
        }
    }
}

/// İzole — task durdur + token revoke
/// Isolated: Suspended'dan farklı, periyot reset ile READY'ye dönmez.
fn isolate_task(id: usize) {
    // SAFETY: Single-hart system, interrupts disabled during boot — no concurrent access.
    unsafe {
        if id < *TASK_COUNT.get() {
            TASKS.get_mut()[id].state = TaskState::Isolated;
            crate::kernel::capability::broker::invalidate_task(TASKS.get_mut()[id].id);
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
        if (states[i] == TaskState::Ready || states[i] == TaskState::Running)
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
}
