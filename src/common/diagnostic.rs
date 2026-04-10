//! Subsystem health check and statistics trait.

/// Runtime istatistikler
#[allow(dead_code)]
pub struct DiagStats {
    pub name: &'static str,
    pub ok: bool,
    pub counter: u32,
    pub error_count: u32,
}

/// Her subsystem bu trait'i implemente edebilir
#[allow(dead_code)]
pub trait Diagnosable {
    fn health_check(&self) -> bool;
    fn stats(&self) -> DiagStats;
}
