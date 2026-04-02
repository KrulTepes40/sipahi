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

// ═══════════════════════════════════════════════════════
// Tipler
// ═══════════════════════════════════════════════════════

/// Başarısızlık modu — 6 mod (doküman §FAILURE POLICY ENGINE)
#[derive(Clone, Copy, PartialEq, Eq)]
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
static mut RESTART_COUNTS: [u8; MAX_TASKS] = [0u8; MAX_TASKS];

/// Restart sayacını sıfırla (task yeniden oluşturulduğunda)
pub fn reset_restart_count(task_id: u8) {
    if (task_id as usize) < MAX_TASKS {
        unsafe { RESTART_COUNTS[task_id as usize] = 0; }
    }
}

/// Restart sayacını oku (test/debug için)
pub fn get_restart_count(task_id: u8) -> u8 {
    if (task_id as usize) < MAX_TASKS {
        unsafe { RESTART_COUNTS[task_id as usize] }
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
pub const fn decide_action(event: u8, restart_count: u8, dal: u8) -> u8 {
    match event {
        // Budget aşımı → RESTART(MAX_RESTART_BUDGET) → DEGRADE
        0 => {
            if restart_count < MAX_RESTART_BUDGET { FailureMode::Restart as u8 }
            else                                  { FailureMode::Degrade as u8 }
        }
        // Stack overflow → RESTART(MAX_RESTART_FAULT) → ISOLATE
        1 => {
            if restart_count < MAX_RESTART_FAULT { FailureMode::Restart as u8 }
            else                                 { FailureMode::Isolate as u8 }
        }
        // WASM trap → RESTART(MAX_RESTART_FAULT) → ISOLATE
        2 => {
            if restart_count < MAX_RESTART_FAULT { FailureMode::Restart as u8 }
            else                                 { FailureMode::Isolate as u8 }
        }
        // Cap violation → ISOLATE (anında, sayaç önemsiz)
        3 => FailureMode::Isolate as u8,
        // IOPMP violation → ISOLATE
        4 => FailureMode::Isolate as u8,
        // PMP integrity fail → SHUTDOWN (anında)
        5 => FailureMode::Shutdown as u8,
        // Watchdog timeout → FAILOVER(MAX_RESTART_WATCHDOG) → DEGRADE
        6 => {
            if restart_count < MAX_RESTART_WATCHDOG { FailureMode::Failover as u8 }
            else                                    { FailureMode::Degrade  as u8 }
        }
        // Deadline miss → DAL'a göre
        7 => {
            match dal {
                0 => FailureMode::Failover as u8, // DAL-A → FAILOVER
                1 => FailureMode::Alert    as u8, // DAL-B → ALERT
                2 => FailureMode::Alert    as u8, // DAL-C → ALERT
                _ => FailureMode::Isolate  as u8, // DAL-D → ISOLATE
            }
        }
        // Çoklu modül çöküşü → SHUTDOWN
        8 => FailureMode::Shutdown as u8,
        // Bilinmeyen olay → ISOLATE (güvenli varsayılan)
        _ => FailureMode::Isolate as u8,
    }
}

// ═══════════════════════════════════════════════════════
// Politika uygulama
// ═══════════════════════════════════════════════════════

/// Politika uygula — restart sayacı günceller, karar döner
/// Scheduler bu kararı alır ve uygular
pub fn apply_policy(task_id: u8, event: PolicyEvent, dal: u8) -> FailureMode {
    let id    = task_id as usize;
    let count = if id < MAX_TASKS {
        unsafe { RESTART_COUNTS[id] }
    } else {
        0
    };

    let action_u8 = decide_action(event as u8, count, dal);
    let action    = u8_to_mode(action_u8);

    // RESTART → sayacı artır (doygun — eskalasyon için MAX tutulur)
    if action == FailureMode::Restart && id < MAX_TASKS {
        unsafe {
            RESTART_COUNTS[id] = RESTART_COUNTS[id].saturating_add(1);
        }
    }

    action
}

/// u8 → FailureMode (yardımcı, repr garantisi ile)
const fn u8_to_mode(v: u8) -> FailureMode {
    match v {
        0 => FailureMode::Restart,
        1 => FailureMode::Isolate,
        2 => FailureMode::Degrade,
        3 => FailureMode::Failover,
        4 => FailureMode::Alert,
        _ => FailureMode::Shutdown,
    }
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
        assert!(decide_action(event, 0, 3) == FailureMode::Restart as u8);
        // İkinci ve sonrası: DEGRADE
        assert!(decide_action(event, 1, 3) == FailureMode::Degrade as u8);
        assert!(decide_action(event, 2, 3) == FailureMode::Degrade as u8);
        let count: u8 = kani::any();
        kani::assume(count >= 1);
        assert!(decide_action(event, count, 3) == FailureMode::Degrade as u8);
    }

    /// Proof 48: Stack overflow → RESTART(3) → ISOLATE eskalasyonu
    #[kani::proof]
    fn stack_overflow_escalation() {
        let event = PolicyEvent::StackOverflow as u8;
        // 0, 1, 2: RESTART
        assert!(decide_action(event, 0, 0) == FailureMode::Restart as u8);
        assert!(decide_action(event, 1, 0) == FailureMode::Restart as u8);
        assert!(decide_action(event, 2, 0) == FailureMode::Restart as u8);
        // 3+: ISOLATE
        assert!(decide_action(event, 3, 0) == FailureMode::Isolate as u8);
        let count: u8 = kani::any();
        kani::assume(count >= 3);
        assert!(decide_action(event, count, 0) == FailureMode::Isolate as u8);
    }

    /// Proof 49: Cap violation → her zaman ISOLATE (sayaç ve DAL önemsiz)
    #[kani::proof]
    fn cap_violation_always_isolate() {
        let event = PolicyEvent::CapViolation as u8;
        let count: u8 = kani::any();
        let dal:   u8 = kani::any();
        kani::assume(dal <= 3);
        assert!(decide_action(event, count, dal) == FailureMode::Isolate as u8);
    }

    /// Proof 50: PMP integrity fail → her zaman SHUTDOWN
    #[kani::proof]
    fn pmp_fail_always_shutdown() {
        let event = PolicyEvent::PmpIntegrityFail as u8;
        let count: u8 = kani::any();
        let dal:   u8 = kani::any();
        kani::assume(dal <= 3);
        assert!(decide_action(event, count, dal) == FailureMode::Shutdown as u8);
    }

    /// Proof 51: Deadline miss — DAL-A FAILOVER, DAL-D ISOLATE
    #[kani::proof]
    fn deadline_miss_dal_specific() {
        let event = PolicyEvent::DeadlineMiss as u8;
        let count: u8 = kani::any();
        // DAL-A → her zaman FAILOVER
        assert!(decide_action(event, count, 0) == FailureMode::Failover as u8);
        // DAL-B → her zaman ALERT
        assert!(decide_action(event, count, 1) == FailureMode::Alert as u8);
        // DAL-C → her zaman ALERT
        assert!(decide_action(event, count, 2) == FailureMode::Alert as u8);
        // DAL-D → her zaman ISOLATE
        assert!(decide_action(event, count, 3) == FailureMode::Isolate as u8);
    }
}
