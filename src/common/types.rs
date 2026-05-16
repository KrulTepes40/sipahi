//! Core type definitions: Q32.32 fixed-point, TaskState, DAL levels.
// U-19 GÖREV 3: blanket #![allow(dead_code)] kaldırıldı — tekil işaretlenir
// Sipahi — Ortak tipler
// Q32.32 fixed-point, task durumu, DAL seviyeleri

/// Q32.32 fixed-point sayı
/// ±2³¹ aralık, ~2.3×10⁻¹⁰ hassasiyet
/// Float YASAK — Sipahi doktrini
#[allow(dead_code)] // Q32.32 alias — WASM compute service v2.0 tarafından kullanılır
pub type Q32 = i64;

/// Task ID (0-7, MAX_TASKS = 8)
pub type TaskId = u8;

/// Kaynak ID (IPC kanalı, compute service, vb.)
pub type ResourceId = u16;

/// Task durumu
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(kani, derive(kani::Arbitrary))]
pub enum TaskState {
    Ready,
    Running,
    Suspended,
    Dead,
    /// Sprint 14: Kalıcı izolasyon — capability revoke + period reset'ten muaf.
    /// Suspended'dan farkı: periyot dolunca Ready'ye DÖNMEZ.
    Isolated,
}

// ═══════════════════════════════════════════════════════
// Newtype wrappers — tip güvenliği için (v1.5'te API'lere entegre edilecek)
// ═══════════════════════════════════════════════════════

/// Type-safe channel identifier (0-7)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(transparent)]
#[allow(dead_code)] // v1.5 newtype API — Sprint genişlemesinde signature'lara entegre edilecek
pub struct ChannelId(pub u8);

/// Type-safe priority level (0=highest, 15=lowest)
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[repr(transparent)]
#[allow(dead_code)] // v1.5 newtype API
pub struct Priority(pub u8);

/// Type-safe DAL level wrapper (0=A, 1=B, 2=C, 3=D)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(transparent)]
#[allow(dead_code)] // v1.5 newtype API
pub struct DalId(pub u8);

/// DO-178C DAL seviyeleri
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(dead_code)] // Public API — production'da `dal: u8` kullanılır, enum cert/doc için
pub enum DalLevel {
    A, // Felaket (silah kontrolü) — %40 CPU, safety factor 1.5×
    B, // Tehlikeli (sensör)       — %30 CPU, safety factor 1.3×
    C, // Önemli (navigasyon)      — %20 CPU, safety factor 1.2×
    D, // Düşük (log, telemetri)   — %10 CPU, safety factor 1.0×
}

impl DalLevel {
    /// DAL'a göre safety factor döner
    #[allow(dead_code)] // Sertifikasyon dökümantasyonu — runtime'da apply_policy kullanır
    pub const fn safety_factor(&self) -> u32 {
        match self {
            DalLevel::A => 150, // 1.5× (×100 çarpanı, integer aritmetik)
            DalLevel::B => 130, // 1.3×
            DalLevel::C => 120, // 1.2×
            DalLevel::D => 100, // 1.0×
        }
    }
}

/// Task creation configuration — named fields prevent parameter mix-ups.
pub struct TaskConfig {
    pub entry: fn() -> !,
    pub priority: u8,
    pub dal: u8,
    pub budget_cycles: u32,
    pub period_ticks: u32,
    /// U-25 FIX-3: SNTM native task flag — manifest-driven multi-region PMP.
    /// Default = false (legacy task_a/task_b: single-NAPOT stack entry 8).
    /// Native task'lar U-26'da native_create_task() ile bu flag'i true set eder.
    /// is_sntm_native=true ise scheduler reload_pmp_profile (entry 8..15) çağırır
    /// ve is_valid_user_ptr multi-region + Access kontrolü aktif olur.
    pub is_sntm_native: bool,
}
