//! Subsystem health check and statistics trait.
#![allow(dead_code)] // v1.5 implementations — trait + struct defined now.

/// Runtime istatistikler
pub struct DiagStats {
    pub name: &'static str,
    pub ok: bool,
    pub counter: u32,
    pub error_count: u32,
}

/// Her subsystem bu trait'i implemente edebilir
pub trait Diagnosable {
    fn health_check(&self) -> bool;
    fn stats(&self) -> DiagStats;
}
