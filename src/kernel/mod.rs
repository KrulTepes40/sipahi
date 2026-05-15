//! Kernel subsystems: scheduler, syscall, capability, policy, memory.
// Sipahi — Kernel Katmanı
// Sprint 4: Scheduler
// Sprint 5: Memory protection (PMP)
// Sprint 10: Policy engine

pub mod scheduler;

#[cfg(not(kani))]
pub mod memory;

pub mod syscall;    // Sprint 7
pub mod capability; // Sprint 9
pub mod policy;     // Sprint 10
pub mod pmp;        // U-24 SNTM Phase 2 — PMP profile types + pure helpers
