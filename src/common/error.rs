//! Kernel error types — every failure is explicit, no silent drops.
#![allow(dead_code)]
// Sipahi — Hata Tipleri
// Safety-critical: her hata açık, sessiz başarısızlık YOK

/// Sipahi kernel hata tipleri
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(kani, derive(kani::Arbitrary))]
pub enum SipahiError {
    /// Capability token doğrulaması başarısız
    CapabilityDenied,
    /// IPC buffer dolu — mesaj YAZILMADI (kayıp yok)
    BufferFull,
    /// Geçersiz syscall numarası
    InvalidSyscall,
    /// Task budget tükendi
    BudgetExhausted,
    /// CRC32 doğrulaması başarısız (IPC veya blackbox)
    IntegrityError,
    /// WASM fuel tükendi
    FuelExhausted,
    /// WASM modül imza doğrulaması başarısız
    ModuleRejected,
    /// PMP integrity check başarısız — KRİTİK
    PmpViolation,
    /// Deadline miss
    DeadlineMiss,
    /// Watchdog timeout — 3 kademeli escalation
    WatchdogTimeout,
    /// Aygıt hazır değil veya başlatılmamış
    DeviceNotReady,
    /// Geçersiz indeks veya parametre
    InvalidParameter,
    /// Token nonce tekrarı (replay attack)
    ReplayDetected,
    /// Geçersiz kullanıcı pointer'ı (kernel belleğine erişim girişimi)
    InvalidPointer,
}

impl SipahiError {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::CapabilityDenied => "capability denied",
            Self::BufferFull       => "buffer full",
            Self::InvalidSyscall   => "invalid syscall",
            Self::BudgetExhausted  => "budget exhausted",
            Self::IntegrityError   => "integrity error",
            Self::FuelExhausted    => "fuel exhausted",
            Self::ModuleRejected   => "module rejected",
            Self::PmpViolation     => "PMP violation",
            Self::DeadlineMiss     => "deadline miss",
            Self::WatchdogTimeout  => "watchdog timeout",
            Self::DeviceNotReady   => "device not ready",
            Self::InvalidParameter => "invalid parameter",
            Self::ReplayDetected   => "replay detected",
            Self::InvalidPointer   => "invalid pointer",
        }
    }
}
