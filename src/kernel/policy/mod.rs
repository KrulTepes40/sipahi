//! 6-mode failure policy engine: RESTART → ISOLATE → DEGRADE → SHUTDOWN.
// Sipahi — Failure Policy Engine (Sprint 10)
// 6 mod: RESTART, ISOLATE, DEGRADE, FAILOVER, ALERT, SHUTDOWN
//
// Escalation tablosu (doküman §FAILURE POLICY ENGINE):
//   Budget aşımı     → RESTART(1)  → DEGRADE
//   Stack overflow   → RESTART(3)  → ISOLATE
//   WASM trap        → RESTART(3)  → ISOLATE
//   Cap violation    → ISOLATE (anında)
//   IOPMP violation  → ISOLATE
//   PMP integrity    → SHUTDOWN (anında)
//   Watchdog timeout → FAILOVER(1) → DEGRADE
//   Deadline miss    → DAL'a göre (A=FAILOVER, B/C=ALERT, D=ISOLATE)
//   Çoklu çöküş      → SHUTDOWN
//
// Mimari:
//   decide_action() — saf sabit fonksiyon (Kani doğrulanabilir, yan etki yok)
//   apply_policy()  — restart sayacı günceller, karar döner
//   Scheduler bu kararı alır ve uygular (döngüsel bağımlılık yok)

use crate::common::config::MAX_TASKS;
use crate::common::sync::SingleHartCell;

// ═══════════════════════════════════════════════════════
// Tipler
// ═══════════════════════════════════════════════════════

/// Başarısızlık modu — 6 mod (doküman §FAILURE POLICY ENGINE)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum FailureMode {
    Restart  = 0, // Task yeniden başlat (max N, sonra eskalasyon)
    Isolate  = 1, // Task durdur, token revoke, IPC kapat
    Degrade  = 2, // DAL-A/B çalışır, C/D durur
    Failover = 3, // Yedek task'a geç (hot-standby, Sprint 11+)
    Alert    = 4, // Operatör bildir, task devam
    Shutdown = 5, // Silahlar safe, aktüatörler nötr, son log
}

/// Politika olayı — hangi koşul tetikledi
#[allow(dead_code)]
#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum PolicyEvent {
    BudgetExhausted  = 0,
    StackOverflow    = 1,
    WasmTrap         = 2,
    CapViolation     = 3,
    IopmpViolation   = 4,
    PmpIntegrityFail = 5,
    WatchdogTimeout  = 6,
    DeadlineMiss     = 7,
    MultiModuleCrash = 8,
}

// ═══════════════════════════════════════════════════════
// Restart sayacı
// ═══════════════════════════════════════════════════════

/// Budget aşımı için maksimum restart sayısı (1 kez → DEGRADE)
pub const MAX_RESTART_BUDGET: u8 = 1;

/// Stack overflow / WASM trap için maksimum restart sayısı (3 kez → ISOLATE)
pub const MAX_RESTART_FAULT: u8 = 3;

/// Watchdog timeout için maksimum FAILOVER sayısı (1 kez → DEGRADE)
pub const MAX_RESTART_WATCHDOG: u8 = 1;

/// Task başına restart sayacı — eskalasyon için
static RESTART_COUNTS: SingleHartCell<[u8; MAX_TASKS]> = SingleHartCell::new([0u8; MAX_TASKS]);

/// Restart sayacını sıfırla (task yeniden oluşturulduğunda)
#[allow(dead_code)]
pub fn reset_restart_count(task_id: u8) {
    if (task_id as usize) < MAX_TASKS {
        // SAFETY: Single-hart system, interrupts disabled during boot — no concurrent access.
        unsafe { (*RESTART_COUNTS.get_mut())[task_id as usize] = 0; }
    }
}

/// Restart sayacını oku (test/debug için)
pub(crate) fn get_restart_count(task_id: u8) -> u8 {
    if (task_id as usize) < MAX_TASKS {
        // SAFETY: Single-hart system, interrupts disabled during boot — no concurrent access.
        unsafe { (*RESTART_COUNTS.get())[task_id as usize] }
    } else {
        0
    }
}

// ═══════════════════════════════════════════════════════
// Saf karar fonksiyonu (Kani doğrulanabilir)
// ═══════════════════════════════════════════════════════

/// Olay + restart_count + dal → mod (saf fonksiyon, yan etki YOK)
/// event: PolicyEvent as u8
/// dal:   0=A 1=B 2=C 3=D
/// Kani'de decide_action ile test edilir (apply_policy'nin yan etkileri yok)
#[must_use = "policy decision must be applied"]
pub const fn decide_action(event: u8, restart_count: u8, dal: u8) -> FailureMode {
    match event {
        0 => {
            if restart_count < MAX_RESTART_BUDGET { FailureMode::Restart }
            else                                  { FailureMode::Degrade }
        }
        1 => {
            if restart_count < MAX_RESTART_FAULT { FailureMode::Restart }
            else                                 { FailureMode::Isolate }
        }
        2 => {
            if restart_count < MAX_RESTART_FAULT { FailureMode::Restart }
            else                                 { FailureMode::Isolate }
        }
        3 => FailureMode::Isolate,
        4 => FailureMode::Isolate,
        5 => FailureMode::Shutdown,
        6 => {
            if restart_count < MAX_RESTART_WATCHDOG { FailureMode::Failover }
            else                                    { FailureMode::Degrade  }
        }
        7 => {
            match dal {
                0 => FailureMode::Failover,
                1 => FailureMode::Alert,
                2 => FailureMode::Alert,
                _ => FailureMode::Isolate,
            }
        }
        8 => FailureMode::Shutdown,
        _ => FailureMode::Isolate,
    }
}

// ═══════════════════════════════════════════════════════
// Politika uygulama
// ═══════════════════════════════════════════════════════

/// Politika uygula — restart sayacı günceller, karar döner
/// Scheduler bu kararı alır ve uygular
pub(crate) fn apply_policy(task_id: u8, event: PolicyEvent, dal: u8) -> FailureMode {
    let id    = task_id as usize;
    let count = if id < MAX_TASKS {
        // SAFETY: Single-hart system, interrupts disabled during boot — no concurrent access.
        unsafe { (*RESTART_COUNTS.get())[id] }
    } else {
        0
    };

    let action1 = decide_action(event as u8, count, dal);
    let action2 = decide_action(event as u8, count, dal);
    // Lockstep: iki çağrı aynı sonucu vermeli — farklıysa bellek bozulması
    let action = if action1 != action2 {
        FailureMode::Shutdown
    } else {
        action1
    };

    // RESTART → sayacı artır (doygun — eskalasyon için MAX tutulur)
    if action == FailureMode::Restart && id < MAX_TASKS {
        // SAFETY: Single-hart system, interrupts disabled during boot — no concurrent access.
        unsafe {
            (*RESTART_COUNTS.get_mut())[id] = (*RESTART_COUNTS.get())[id].saturating_add(1);
        }
    }

    action
}

// ═══════════════════════════════════════════════════════
// Kani — Sprint 10 (Proof 47-51)
// ═══════════════════════════════════════════════════════

#[cfg(kani)]
mod verification {
    use super::*;

    /// Proof 47: Budget aşımı → RESTART(1) → DEGRADE eskalasyonu
    #[kani::proof]
    fn budget_exhausted_escalation() {
        let event = PolicyEvent::BudgetExhausted as u8;
        // İlk olay: RESTART
        assert!(decide_action(event, 0, 3) == FailureMode::Restart);
        // İkinci ve sonrası: DEGRADE
        assert!(decide_action(event, 1, 3) == FailureMode::Degrade);
        assert!(decide_action(event, 2, 3) == FailureMode::Degrade);
        let count: u8 = kani::any();
        kani::assume(count >= 1);
        assert!(decide_action(event, count, 3) == FailureMode::Degrade);
    }

    /// Proof 48: Stack overflow → RESTART(3) → ISOLATE eskalasyonu
    #[kani::proof]
    fn stack_overflow_escalation() {
        let event = PolicyEvent::StackOverflow as u8;
        // 0, 1, 2: RESTART
        assert!(decide_action(event, 0, 0) == FailureMode::Restart);
        assert!(decide_action(event, 1, 0) == FailureMode::Restart);
        assert!(decide_action(event, 2, 0) == FailureMode::Restart);
        // 3+: ISOLATE
        assert!(decide_action(event, 3, 0) == FailureMode::Isolate);
        let count: u8 = kani::any();
        kani::assume(count >= 3);
        assert!(decide_action(event, count, 0) == FailureMode::Isolate);
    }

    /// Proof 49: Cap violation → her zaman ISOLATE (sayaç ve DAL önemsiz)
    #[kani::proof]
    fn cap_violation_always_isolate() {
        let event = PolicyEvent::CapViolation as u8;
        let count: u8 = kani::any();
        let dal:   u8 = kani::any();
        kani::assume(dal <= 3);
        assert!(decide_action(event, count, dal) == FailureMode::Isolate);
    }

    /// Proof 50: PMP integrity fail → her zaman SHUTDOWN
    #[kani::proof]
    fn pmp_fail_always_shutdown() {
        let event = PolicyEvent::PmpIntegrityFail as u8;
        let count: u8 = kani::any();
        let dal:   u8 = kani::any();
        kani::assume(dal <= 3);
        assert!(decide_action(event, count, dal) == FailureMode::Shutdown);
    }

    /// Proof 51: Deadline miss — DAL-A FAILOVER, DAL-D ISOLATE
    #[kani::proof]
    fn deadline_miss_dal_specific() {
        let event = PolicyEvent::DeadlineMiss as u8;
        let count: u8 = kani::any();
        // DAL-A → her zaman FAILOVER
        assert!(decide_action(event, count, 0) == FailureMode::Failover);
        // DAL-B → her zaman ALERT
        assert!(decide_action(event, count, 1) == FailureMode::Alert);
        // DAL-C → her zaman ALERT
        assert!(decide_action(event, count, 2) == FailureMode::Alert);
        // DAL-D → her zaman ISOLATE
        assert!(decide_action(event, count, 3) == FailureMode::Isolate);
    }

    /// Proof 109: PMP fail (event=5) → HER ZAMAN Shutdown
    #[kani::proof]
    fn pmp_fail_always_shutdown_any_input() {
        let rc: u8 = kani::any();
        let dal: u8 = kani::any();
        kani::assume(dal <= 3);
        assert!(decide_action(5, rc, dal) == FailureMode::Shutdown);
    }

    /// Proof 110: Budget escalation: ilk → Restart, tekrarlı → not Restart
    #[kani::proof]
    fn budget_escalation_restart_then_degrade() {
        let dal: u8 = kani::any();
        kani::assume(dal <= 3);
        assert!(decide_action(0, 0, dal) == FailureMode::Restart);
        assert!(decide_action(0, 255, dal) != FailureMode::Restart);
    }

    /// Proof 111: CapViolation (event=3) → HER ZAMAN Isolate
    #[kani::proof]
    fn cap_violation_always_isolate_any_dal() {
        let rc: u8 = kani::any();
        let dal: u8 = kani::any();
        kani::assume(dal <= 3);
        assert!(decide_action(3, rc, dal) == FailureMode::Isolate);
    }

    /// Proof 112: MultiModuleCrash (event=8) → Shutdown
    #[kani::proof]
    fn multi_module_crash_shutdown() {
        let rc: u8 = kani::any();
        let dal: u8 = kani::any();
        kani::assume(dal <= 3);
        assert!(decide_action(8, rc, dal) == FailureMode::Shutdown);
    }

    /// Proof 113: Bilinmeyen event (>8) → Isolate (fail-safe default)
    #[kani::proof]
    fn unknown_event_defaults_isolate() {
        let event: u8 = kani::any();
        kani::assume(event > 8);
        let rc: u8 = kani::any();
        let dal: u8 = kani::any();
        kani::assume(dal <= 3);
        assert!(decide_action(event, rc, dal) == FailureMode::Isolate);
    }

    /// Proof 153: PMP(5) ve MultiModule(8) → Shutdown (tüm rc/dal)
    #[kani::proof]
    fn shutdown_events_always_shutdown() {
        let rc: u8 = kani::any();
        let dal: u8 = kani::any();
        kani::assume(dal <= 3);
        assert!(decide_action(5, rc, dal) == FailureMode::Shutdown);
        assert!(decide_action(8, rc, dal) == FailureMode::Shutdown);
    }

    /// Budget Degrade → rc >= MAX_RESTART_BUDGET olmalı
    #[kani::proof]
    fn budget_degrade_requires_exhausted_restarts() {
        let rc: u8 = kani::any();
        let dal: u8 = kani::any();
        kani::assume(dal <= 3);
        let action = decide_action(0, rc, dal); // event=0 → BudgetExhausted
        if matches!(action, FailureMode::Degrade) {
            assert!(rc >= MAX_RESTART_BUDGET);
        }
    }

    /// Watchdog Degrade → rc >= MAX_RESTART_WATCHDOG olmalı
    #[kani::proof]
    fn watchdog_degrade_requires_exhausted_restarts() {
        let rc: u8 = kani::any();
        let dal: u8 = kani::any();
        kani::assume(dal <= 3);
        let action = decide_action(6, rc, dal);
        if matches!(action, FailureMode::Degrade) {
            assert!(rc >= MAX_RESTART_WATCHDOG);
        }
    }

    /// Livelock freedom: 10 ardışık budget çöküşte terminal state'e ulaşılır
    #[kani::proof]
    #[kani::unwind(12)]
    fn policy_never_livelocks_on_repeated_failure() {
        let dal: u8 = kani::any();
        kani::assume(dal <= 3);
        let mut reached_terminal = false;
        let mut rc: u8 = 0;
        while rc < 10 {
            let action = decide_action(0, rc, dal); // BudgetExhausted
            if matches!(action, FailureMode::Degrade | FailureMode::Isolate | FailureMode::Shutdown) {
                reached_terminal = true;
            }
            rc = rc.saturating_add(1);
        }
        assert!(reached_terminal);
    }
}
