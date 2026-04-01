// Sipahi — Kernel Katmanı
// Sprint 4: Scheduler
// Sprint 5: Memory protection (PMP)

pub mod scheduler;

#[cfg(not(kani))]
pub mod memory;

pub mod syscall;         // Sprint 7
// pub mod capability;   // Sprint 9
// pub mod policy;       // Sprint 10
