//! Core type definitions: Q32.32 fixed-point, TaskState, DAL levels.
#![allow(dead_code)] // API types — used by Kani proofs and future U-mode tasks.
// Sipahi — Ortak tipler
// Q32.32 fixed-point, task durumu, DAL seviyeleri

/// Q32.32 fixed-point sayı
/// ±2³¹ aralık, ~2.3×10⁻¹⁰ hassasiyet
/// Float YASAK — Sipahi doktrini
pub type Q32 = i64;

/// Task ID (0-7, MAX_TASKS = 8)
pub type TaskId = u8;

/// Kaynak ID (IPC kanalı, compute service, vb.)
pub type ResourceId = u16;

/// Task durumu
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TaskState {
    Ready,
    Running,
    Suspended,
    Dead,
    /// Sprint 14: Kalıcı izolasyon — capability revoke + period reset'ten muaf.
    /// Suspended'dan farkı: periyot dolunca Ready'ye DÖNMEZ.
    Isolated,
}

/// DO-178C DAL seviyeleri
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DalLevel {
    A, // Felaket (silah kontrolü) — %40 CPU, safety factor 1.5×
    B, // Tehlikeli (sensör)       — %30 CPU, safety factor 1.3×
    C, // Önemli (navigasyon)      — %20 CPU, safety factor 1.2×
    D, // Düşük (log, telemetri)   — %10 CPU, safety factor 1.0×
}

impl DalLevel {
    /// DAL'a göre safety factor döner
    pub const fn safety_factor(&self) -> u32 {
        match self {
            DalLevel::A => 150, // 1.5× (×100 çarpanı, integer aritmetik)
            DalLevel::B => 130, // 1.3×
            DalLevel::C => 120, // 1.2×
            DalLevel::D => 100, // 1.0×
        }
    }
}
